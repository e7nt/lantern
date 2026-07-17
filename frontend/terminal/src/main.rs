use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as TerminalEvent, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};
use lantern_diagnostics::{
    DaemonState as DiagnosticDaemonState, bundle_from_stderr, summarize_stderr,
};
use lantern_protocol::{
    BoundedTail, ChangeProposal, Event, Evidence, EvidenceSource, MAX_DIAGNOSTIC_BYTES,
    MAX_SELECTION_BYTES, PROTOCOL_VERSION, Request, SelectionContext, SymbolContext,
    SymbolContextExport, search_term,
};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(all(unix, test))]
use std::os::unix::fs::PermissionsExt;

const CANVAS: Color = Color::Rgb {
    r: 58,
    g: 42,
    b: 77,
};
const TEXT: Color = Color::Rgb {
    r: 199,
    g: 184,
    b: 224,
};
const MUTED: Color = Color::Rgb {
    r: 136,
    g: 108,
    b: 156,
};
const ACCENT: Color = Color::Rgb {
    r: 127,
    g: 201,
    b: 171,
};
const LINK: Color = Color::Rgb {
    r: 199,
    g: 141,
    b: 252,
};
const HORIZONTAL_PADDING: u16 = 2;
const DAEMON_STARTUP_TIMEOUT: Duration = Duration::from_secs(2);

enum Input {
    Terminal(TerminalEvent),
    Daemon(Event),
    DaemonClosed { diagnostics: String },
    DaemonStartupTimeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DaemonState {
    Starting,
    Ready,
    Unavailable,
}

#[derive(Clone)]
enum TranscriptItem {
    Line(String),
    Answer { id: u64, text: String },
    Evidence(Evidence),
}

struct UiState {
    input: Vec<char>,
    cursor: usize,
    transcript: Vec<TranscriptItem>,
    scroll_from_bottom: usize,
    activity: Option<String>,
    daemon: DaemonState,
    active_id: Option<u64>,
    accepted_id: Option<u64>,
    next_id: u64,
    pending_symbol_context: Option<SymbolContext>,
    navigated_for: Option<u64>,
    selected_evidence: Option<usize>,
}

impl UiState {
    fn new(_repository: &Path) -> Self {
        Self {
            input: Vec::new(),
            cursor: 0,
            transcript: Vec::new(),
            scroll_from_bottom: 0,
            activity: None,
            daemon: DaemonState::Starting,
            active_id: None,
            accepted_id: None,
            next_id: 1,
            pending_symbol_context: None,
            navigated_for: None,
            selected_evidence: None,
        }
    }

    fn line(&mut self, message: impl Into<String>) {
        self.transcript
            .push(TranscriptItem::Line(clean_text(&message.into())));
        self.scroll_from_bottom = 0;
    }

    fn answer_delta(&mut self, id: u64, delta: &str) {
        let delta = clean_text(delta);
        match self.transcript.last_mut() {
            Some(TranscriptItem::Answer {
                id: answer_id,
                text,
            }) if *answer_id == id => text.push_str(&delta),
            _ => self
                .transcript
                .push(TranscriptItem::Answer { id, text: delta }),
        }
        self.scroll_from_bottom = 0;
    }

    fn take_input(&mut self) -> String {
        self.cursor = 0;
        self.input.drain(..).collect()
    }

    fn begin_operation(&mut self) -> Option<u64> {
        if self.daemon != DaemonState::Ready || self.active_id.is_some() {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.active_id = Some(id);
        self.accepted_id = None;
        self.selected_evidence = None;
        Some(id)
    }

    fn evidence_count(&self) -> usize {
        self.transcript
            .iter()
            .filter(|item| matches!(item, TranscriptItem::Evidence(_)))
            .count()
    }

    fn evidence(&self, index: usize) -> Option<&Evidence> {
        self.transcript
            .iter()
            .filter_map(|item| match item {
                TranscriptItem::Evidence(evidence) => Some(evidence),
                _ => None,
            })
            .nth(index)
    }

    fn select_evidence(&mut self, direction: i8) {
        let count = self.evidence_count();
        if count == 0 {
            self.selected_evidence = None;
            return;
        }
        self.selected_evidence = Some(match (self.selected_evidence, direction) {
            (Some(0), direction) if direction < 0 => count - 1,
            (Some(index), direction) if direction < 0 => index - 1,
            (Some(index), _) => (index + 1) % count,
            (None, direction) if direction < 0 => count - 1,
            (None, _) => 0,
        });
        self.scroll_from_bottom = 0;
    }

    fn daemon_failed(&mut self, reason: &str, diagnostics: &str) {
        if self.daemon == DaemonState::Unavailable {
            return;
        }
        self.daemon = DaemonState::Unavailable;
        self.active_id = None;
        self.accepted_id = None;
        self.navigated_for = None;
        self.line(format!("Agent unavailable: {reason}"));
        let diagnostics = diagnostic_summary(diagnostics);
        if !diagnostics.is_empty() {
            self.line(format!("Daemon diagnostics: {diagnostics}"));
        }
        self.line(
            "Editing and Git remain available. Restart the Lantern session to restore the agent.",
        );
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            Show,
            ResetColor,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

struct Layout {
    input_row: u16,
    evidence_rows: Vec<(u16, usize, Evidence)>,
}

struct TranscriptRow {
    text: String,
    evidence: Option<(usize, Evidence)>,
    muted: bool,
}

fn send_request(stdin: &Arc<Mutex<BufWriter<ChildStdin>>>, request: &Request) -> io::Result<()> {
    let mut stdin = stdin.lock().expect("daemon stdin lock");
    serde_json::to_writer(&mut *stdin, request)?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

fn clean_text(text: &str) -> String {
    text.chars()
        .filter_map(|character| match character {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            character if character.is_control() => None,
            character => Some(character),
        })
        .collect()
}

fn diagnostic_summary(text: &str) -> String {
    summarize_stderr(text)
}

fn truncate(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for logical_line in text.split('\n') {
        let characters: Vec<_> = logical_line.chars().collect();
        if characters.is_empty() {
            wrapped.push(String::new());
            continue;
        }
        for chunk in characters.chunks(width) {
            wrapped.push(chunk.iter().collect());
        }
    }
    wrapped
}

fn evidence_source_text(source: EvidenceSource) -> (&'static str, &'static str) {
    match source {
        EvidenceSource::Selection => ("Selected code", "exact code highlighted in Helix"),
        EvidenceSource::Definition => ("Definition", "symbol definition resolved by Helix"),
        EvidenceSource::Reference => ("Reference", "bounded symbol usage resolved by Helix"),
        EvidenceSource::LiteralMatch => ("Exact match", "local repository text match"),
    }
}

fn flattened_transcript(state: &UiState, width: usize) -> Vec<TranscriptRow> {
    let mut rows = Vec::new();
    let mut evidence_index = 0;
    for item in &state.transcript {
        match item {
            TranscriptItem::Line(line) => {
                rows.extend(
                    wrap_text(line, width)
                        .into_iter()
                        .map(|text| TranscriptRow {
                            text,
                            evidence: None,
                            muted: true,
                        }),
                );
            }
            TranscriptItem::Answer { text, .. } => {
                rows.extend(
                    wrap_text(text, width)
                        .into_iter()
                        .map(|text| TranscriptRow {
                            text,
                            evidence: None,
                            muted: false,
                        }),
                );
            }
            TranscriptItem::Evidence(evidence) => {
                let (source, reason) = evidence_source_text(evidence.source);
                let label = format!(
                    "↗ {}:{}:{}-{}:{}  {source} · {reason}",
                    evidence.relative_path.display(),
                    evidence.start_line,
                    evidence.start_column,
                    evidence.end_line,
                    evidence.end_column
                );
                rows.extend(
                    wrap_text(&label, width)
                        .into_iter()
                        .map(|text| TranscriptRow {
                            text,
                            evidence: Some((evidence_index, evidence.clone())),
                            muted: false,
                        }),
                );
                evidence_index += 1;
            }
        }
    }
    rows
}

fn render(state: &UiState) -> io::Result<Layout> {
    let (width, height) = terminal::size()?;
    let content_width = width.saturating_sub(HORIZONTAL_PADDING.saturating_mul(2));
    let width_usize = usize::from(content_width.max(1));
    let input_row = height.saturating_sub(1);
    let transcript_start = 0;
    let transcript_height = input_row.saturating_sub(transcript_start) as usize;
    let mut stdout = io::stdout();
    queue!(
        stdout,
        Hide,
        SetBackgroundColor(CANVAS),
        SetForegroundColor(TEXT),
        MoveTo(0, 0),
        Clear(ClearType::All)
    )?;

    let rows = flattened_transcript(state, width_usize);
    let max_scroll = rows.len().saturating_sub(transcript_height);
    let scroll = state.scroll_from_bottom.min(max_scroll);
    let end = rows.len().saturating_sub(scroll);
    let start = end.saturating_sub(transcript_height);
    let mut evidence_rows = Vec::new();
    for (offset, transcript_row) in rows[start..end].iter().enumerate() {
        let row = transcript_start + offset as u16;
        if let Some((index, evidence)) = &transcript_row.evidence {
            evidence_rows.push((row, *index, evidence.clone()));
            queue!(stdout, SetForegroundColor(LINK))?;
            if state.selected_evidence == Some(*index) {
                queue!(stdout, SetAttribute(Attribute::Reverse))?;
            }
        } else if transcript_row.muted {
            queue!(stdout, SetForegroundColor(MUTED))?;
        }
        queue!(
            stdout,
            MoveTo(HORIZONTAL_PADDING, row),
            Print(truncate(&transcript_row.text, width_usize))
        )?;
        if let Some((index, _)) = &transcript_row.evidence {
            if state.selected_evidence == Some(*index) {
                queue!(stdout, SetAttribute(Attribute::NoReverse))?;
            }
            queue!(stdout, SetForegroundColor(TEXT))?;
        } else if transcript_row.muted {
            queue!(stdout, SetForegroundColor(TEXT))?;
        }
    }

    let prompt = match (state.daemon, state.active_id) {
        (DaemonState::Starting, _) => "›  Starting local agent…".to_owned(),
        (DaemonState::Unavailable, _) => format!(
            "› {}  · Agent unavailable · /diagnostics exports metadata",
            state.input.iter().collect::<String>()
        ),
        (DaemonState::Ready, Some(_)) => format!(
            "● {}  ·  Esc cancel",
            state.activity.as_deref().unwrap_or("Thinking…")
        ),
        (DaemonState::Ready, None) if state.selected_evidence.is_some() => {
            "›  ↑↓ choose evidence · Enter opens in Helix · Esc returns to prompt".to_owned()
        }
        (DaemonState::Ready, None) => format!("› {}", state.input.iter().collect::<String>()),
    };
    queue!(
        stdout,
        MoveTo(HORIZONTAL_PADDING, input_row),
        SetForegroundColor(ACCENT),
        Print(truncate(&prompt, width_usize)),
        SetForegroundColor(TEXT)
    )?;
    if state.daemon != DaemonState::Starting
        && state.active_id.is_none()
        && state.selected_evidence.is_none()
    {
        let cursor_column = usize::from(HORIZONTAL_PADDING)
            .saturating_add(2)
            .saturating_add(state.cursor)
            .min(usize::from(width).saturating_sub(1));
        queue!(stdout, MoveTo(cursor_column as u16, input_row), Show)?;
    }
    stdout.flush()?;

    Ok(Layout {
        input_row,
        evidence_rows,
    })
}

fn read_symbol_context_file(path: &Path, wait: bool) -> Result<SymbolContext, String> {
    if wait {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    let bytes = fs::read(path).map_err(|cause| {
        format!("Helix did not provide LSP symbol context within five seconds: {cause}")
    })?;
    let _ = fs::remove_file(path);
    if bytes.len() > MAX_SELECTION_BYTES + 16 * 1024 {
        return Err("LSP symbol context file exceeds its bounded size".into());
    }
    let export: SymbolContextExport = serde_json::from_slice(&bytes)
        .map_err(|cause| format!("Helix returned invalid LSP symbol context: {cause}"))?;
    export.into_context()
}

fn capture_symbol_context(path: &Path) -> Result<SymbolContext, String> {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(format!("cannot clear stale selection context: {cause}")),
    }
    let status = Command::new("lantern-capture-selection")
        .status()
        .map_err(|cause| format!("cannot start the Helix selection bridge: {cause}"))?;
    if !status.success() {
        return Err(format!("selection bridge exited with {status}"));
    }
    read_symbol_context_file(path, true)
}

fn symbol_context_for_question(
    state: &mut UiState,
    selection_path: &Path,
) -> Result<SymbolContext, String> {
    if let Some(context) = state.pending_symbol_context.take() {
        return Ok(context);
    }
    if selection_path.exists() {
        return read_symbol_context_file(selection_path, false);
    }
    Err("no editor selection is available".into())
}

fn selection_for_question(
    state: &mut UiState,
    selection_path: &Path,
) -> Result<SelectionContext, String> {
    symbol_context_for_question(state, selection_path).map(|context| context.selection)
}

fn show_proposal(selection_path: &Path, proposal: &ChangeProposal) -> Result<(), String> {
    let directory = selection_path
        .parent()
        .ok_or("selection bridge has no parent directory")?;
    let before = directory.join("proposal.before");
    let after = directory.join("proposal.after");
    fs::write(&before, &proposal.selection.text)
        .map_err(|cause| format!("cannot write preview input: {cause}"))?;
    fs::write(&after, &proposal.replacement)
        .map_err(|cause| format!("cannot write preview output: {cause}"))?;
    let status = Command::new("lantern-preview-diff")
        .arg(&before)
        .arg(&after)
        .arg(&proposal.selection.relative_path)
        .status()
        .map_err(|cause| format!("cannot open change preview: {cause}"));
    let _ = fs::remove_file(before);
    let _ = fs::remove_file(after);
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("change preview exited with {status}")),
        Err(message) => Err(message),
    }
}

fn spawn_terminal_reader(sender: Sender<Input>) {
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if sender.send(Input::Terminal(event)).is_err() {
                break;
            }
        }
    });
}

fn spawn_diagnostic_reader(
    mut reader: impl io::Read + Send + 'static,
    diagnostics: Arc<Mutex<BoundedTail>>,
) {
    thread::spawn(move || {
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => diagnostics
                    .lock()
                    .expect("daemon diagnostics lock")
                    .push(&chunk[..read]),
                Err(_) => break,
            }
        }
    });
}

fn spawn_daemon_reader(
    reader: impl io::Read + Send + 'static,
    diagnostics: Arc<Mutex<BoundedTail>>,
    sender: Sender<Input>,
) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else { break };
            match serde_json::from_str::<Event>(&line) {
                Ok(event) => {
                    if sender.send(Input::Daemon(event)).is_err() {
                        break;
                    }
                }
                Err(cause) => {
                    let _ = sender.send(Input::Daemon(Event::Error {
                        id: None,
                        message: format!("invalid daemon event: {cause}"),
                        recovery: "rebuild and restart Lantern".into(),
                    }));
                }
            }
        }
        let diagnostics = diagnostics.lock().expect("daemon diagnostics lock").text();
        let _ = sender.send(Input::DaemonClosed { diagnostics });
    });
}

fn spawn_startup_deadline(sender: Sender<Input>) {
    thread::spawn(move || {
        thread::sleep(DAEMON_STARTUP_TIMEOUT);
        let _ = sender.send(Input::DaemonStartupTimeout);
    });
}

fn navigate(evidence: &Evidence) -> io::Result<()> {
    let status = Command::new("lantern-open-range")
        .arg(&evidence.relative_path)
        .arg(evidence.start_line.to_string())
        .arg(evidence.start_column.to_string())
        .arg(evidence.end_line.to_string())
        .arg(evidence.end_column.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "navigation bridge exited with {status}"
        )))
    }
}

fn prepare_selection(state: &mut UiState, selection_path: &Path) {
    match capture_symbol_context(selection_path) {
        Ok(context) => {
            let selection = &context.selection;
            let location = format!(
                "Symbol context ready: {}:{}:{}-{}:{} (1 definition, {} references)",
                selection.relative_path.display(),
                selection.start_line,
                selection.start_column,
                selection.end_line,
                selection.end_column,
                context.references.len(),
            );
            state.pending_symbol_context = Some(context);
            state.line(location);
        }
        Err(message) => state.line(format!("Symbol context failed: {message}")),
    }
}

fn start_agent_question(
    state: &mut UiState,
    repository: &Path,
    selection_path: &Path,
    daemon_stdin: &Arc<Mutex<BufWriter<ChildStdin>>>,
    query: &str,
) -> io::Result<()> {
    let query = query.trim();
    if query.is_empty() {
        state.line("Type a question about the selection, then press Enter.");
        return Ok(());
    }
    let Some(id) = state.begin_operation() else {
        state.line("The agent is already working.");
        return Ok(());
    };
    match symbol_context_for_question(state, selection_path) {
        Ok(context) => {
            send_request(
                daemon_stdin,
                &Request::AskAgentSymbol {
                    id,
                    repository: repository.to_owned(),
                    query: query.to_owned(),
                    context,
                },
            )?;
        }
        Err(_) => send_request(
            daemon_stdin,
            &Request::AskAgent {
                id,
                repository: repository.to_owned(),
                query: query.to_owned(),
            },
        )?,
    }
    Ok(())
}

fn cancel_active(
    state: &mut UiState,
    daemon_stdin: &Arc<Mutex<BufWriter<ChildStdin>>>,
) -> io::Result<()> {
    if let Some(id) = state.active_id {
        send_request(daemon_stdin, &Request::Cancel { id })?;
        state.line(format!("Cancelling operation {id}…"));
    } else {
        state.line("No operation is active.");
    }
    Ok(())
}

fn open_git(state: &mut UiState) {
    match Command::new("lantern-lazygit").status() {
        Ok(status) if status.success() => state.line("Returned from Lazygit."),
        Ok(status) => state.line(format!("Lazygit exited with {status}.")),
        Err(cause) => state.line(format!("Could not start Lazygit: {cause}")),
    }
}

fn diagnostic_state(state: DaemonState) -> DiagnosticDaemonState {
    match state {
        DaemonState::Starting => DiagnosticDaemonState::Starting,
        DaemonState::Ready => DiagnosticDaemonState::Ready,
        DaemonState::Unavailable => DiagnosticDaemonState::Unavailable,
    }
}

fn write_diagnostic_bundle(
    directory: &Path,
    daemon_state: DaemonState,
    stderr: &str,
) -> Result<PathBuf, String> {
    let bundle = bundle_from_stderr(
        stderr,
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
        diagnostic_state(daemon_state),
    );
    let mut encoded = serde_json::to_vec_pretty(&bundle)
        .map_err(|cause| format!("cannot encode diagnostic bundle: {cause}"))?;
    encoded.push(b'\n');
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    for attempt in 0..16 {
        let path = directory.join(format!(
            "lantern-diagnostics-{}-{timestamp}-{attempt}.json",
            std::process::id()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(mut file) => {
                if let Err(cause) = file.write_all(&encoded).and_then(|()| file.flush()) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(format!("cannot finish diagnostic bundle: {cause}"));
                }
                return Ok(path);
            }
            Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(cause) => {
                return Err(format!(
                    "cannot create a diagnostic bundle in {}: {cause}",
                    directory.display()
                ));
            }
        }
    }
    Err("cannot allocate a unique diagnostic bundle name".into())
}

fn export_diagnostics(state: &mut UiState, diagnostics: &Arc<Mutex<BoundedTail>>) {
    let stderr = diagnostics.lock().expect("daemon diagnostics lock").text();
    match write_diagnostic_bundle(&env::temp_dir(), state.daemon, &stderr) {
        Ok(path) => state.line(format!(
            "Diagnostic metadata exported to {}. Prompts, source, paths, environment values, provider stderr, and unstructured output were excluded.",
            path.display()
        )),
        Err(message) => state.line(format!("Diagnostic export failed: {message}")),
    }
}

fn handle_line(
    line: String,
    state: &mut UiState,
    repository: &Path,
    selection_path: &Path,
    daemon_stdin: &Arc<Mutex<BufWriter<ChildStdin>>>,
    diagnostics: &Arc<Mutex<BoundedTail>>,
) -> io::Result<bool> {
    let line = line.trim().to_owned();
    if line == "/quit" {
        if state.active_id.is_some() {
            state.line("Cancel the active agent turn before quitting.");
            return Ok(false);
        }
        return Ok(true);
    }
    if line == "/cancel" {
        cancel_active(state, daemon_stdin)?;
        return Ok(false);
    }
    if line == "/git" {
        open_git(state);
        return Ok(false);
    }
    if line == "/refresh" || line.is_empty() {
        return Ok(false);
    }
    if line == "/diagnostics" {
        export_diagnostics(state, diagnostics);
        return Ok(false);
    }
    if state.active_id.is_some() {
        state.line("The agent is working. Click Cancel to interrupt it.");
        return Ok(false);
    }
    if state.daemon != DaemonState::Ready {
        let message = match state.daemon {
            DaemonState::Starting => "The local agent is still starting.",
            DaemonState::Unavailable => {
                "The local agent is unavailable. Restart the Lantern session to retry."
            }
            DaemonState::Ready => unreachable!(),
        };
        state.line(message);
        return Ok(false);
    }
    if let Some(query) = line.strip_prefix("/agent ") {
        start_agent_question(state, repository, selection_path, daemon_stdin, query)?;
    } else if let Some(query) = line.strip_prefix("/ask ") {
        match selection_for_question(state, selection_path) {
            Ok(selection) => {
                let Some(id) = state.begin_operation() else {
                    return Ok(false);
                };
                send_request(
                    daemon_stdin,
                    &Request::AskSelection {
                        id,
                        repository: repository.to_owned(),
                        query: query.trim().to_owned(),
                        selection,
                    },
                )?;
            }
            Err(message) => state.line(format!("Selection failed: {message}")),
        }
    } else if let Some(replacement) = line.strip_prefix("/preview ") {
        match selection_for_question(state, selection_path) {
            Ok(selection) => {
                let Some(id) = state.begin_operation() else {
                    return Ok(false);
                };
                send_request(
                    daemon_stdin,
                    &Request::PreviewSelection {
                        id,
                        repository: repository.to_owned(),
                        selection,
                        replacement: replacement.to_owned(),
                    },
                )?;
            }
            Err(message) => state.line(format!("Selection failed: {message}")),
        }
    } else if line.starts_with("/show ") {
        match search_term(&line) {
            Ok(query) => {
                let Some(id) = state.begin_operation() else {
                    return Ok(false);
                };
                send_request(
                    daemon_stdin,
                    &Request::Ask {
                        id,
                        repository: repository.to_owned(),
                        query,
                    },
                )?;
            }
            Err(message) => state.line(message),
        }
    } else if line.starts_with('/') {
        state.line("Unknown diagnostic command.");
    } else {
        start_agent_question(state, repository, selection_path, daemon_stdin, &line)?;
    }
    Ok(false)
}

fn handle_key(
    key: KeyEvent,
    state: &mut UiState,
    repository: &Path,
    selection_path: &Path,
    daemon_stdin: &Arc<Mutex<BufWriter<ChildStdin>>>,
    diagnostics: &Arc<Mutex<BoundedTail>>,
) -> io::Result<bool> {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return Ok(false);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('a') {
        if state.daemon == DaemonState::Ready && state.active_id.is_none() {
            prepare_selection(state, selection_path);
        }
        return Ok(false);
    }
    if is_quit_shortcut(key, state.active_id.is_some(), state.input.is_empty()) {
        return Ok(true);
    }
    if key.code == KeyCode::Esc && state.active_id.is_some() {
        cancel_active(state, daemon_stdin)?;
        return Ok(false);
    }
    if key.code == KeyCode::Esc && state.selected_evidence.take().is_some() {
        return Ok(false);
    }
    if state.active_id.is_some() {
        return Ok(false);
    }
    if state.daemon == DaemonState::Starting {
        return Ok(false);
    }
    if state.input.is_empty() && matches!(key.code, KeyCode::Up | KeyCode::Down) {
        state.select_evidence(if key.code == KeyCode::Up { -1 } else { 1 });
        return Ok(false);
    }
    if key.code == KeyCode::Enter
        && let Some(index) = state.selected_evidence
    {
        if let Some(evidence) = state.evidence(index)
            && let Err(cause) = navigate(evidence)
        {
            state.line(format!("Navigation failed: {cause}"));
        }
        return Ok(false);
    }
    state.selected_evidence = None;
    match key.code {
        KeyCode::Enter => {
            let line = state.take_input();
            handle_line(
                line,
                state,
                repository,
                selection_path,
                daemon_stdin,
                diagnostics,
            )
        }
        KeyCode::Char(character)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            state.input.insert(state.cursor, character);
            state.cursor += 1;
            Ok(false)
        }
        KeyCode::Backspace if state.cursor > 0 => {
            state.cursor -= 1;
            state.input.remove(state.cursor);
            Ok(false)
        }
        KeyCode::Delete if state.cursor < state.input.len() => {
            state.input.remove(state.cursor);
            Ok(false)
        }
        KeyCode::Left => {
            state.cursor = state.cursor.saturating_sub(1);
            Ok(false)
        }
        KeyCode::Right => {
            state.cursor = (state.cursor + 1).min(state.input.len());
            Ok(false)
        }
        KeyCode::Home => {
            state.cursor = 0;
            Ok(false)
        }
        KeyCode::End => {
            state.cursor = state.input.len();
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn is_quit_shortcut(key: KeyEvent, agent_active: bool, input_empty: bool) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && key.code == KeyCode::Char('d')
        && !agent_active
        && input_empty
}

fn handle_mouse(mouse: MouseEvent, layout: &Layout, state: &mut UiState) -> io::Result<bool> {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(3);
        }
        MouseEventKind::ScrollDown => {
            state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(3);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some((_, index, evidence)) = layout
                .evidence_rows
                .iter()
                .find(|(row, _, _)| *row == mouse.row)
            {
                state.selected_evidence = Some(*index);
                if let Err(cause) = navigate(evidence) {
                    state.line(format!("Navigation failed: {cause}"));
                }
            } else if mouse.row == layout.input_row
                && state.daemon != DaemonState::Starting
                && state.active_id.is_none()
            {
                state.selected_evidence = None;
                let input_origin = HORIZONTAL_PADDING.saturating_add(2);
                state.cursor =
                    usize::from(mouse.column.saturating_sub(input_origin)).min(state.input.len());
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_daemon_event(event: Event, state: &mut UiState, selection_path: &Path) -> io::Result<()> {
    match event {
        Event::Initialized { .. } => {
            state.daemon = DaemonState::Ready;
            state.transcript.clear();
            state.activity = Some("Opening workbench…".into());
        }
        Event::WorkbenchOpened { .. } => {
            state.activity = None;
        }
        Event::WorkbenchOpenFailed { message, recovery } => {
            state.line(format!("Could not open workbench: {message}"));
            state.line(format!("Recovery: {recovery}"));
        }
        Event::Accepted { id } => {
            if state.active_id == Some(id) {
                state.accepted_id = Some(id);
            }
        }
        Event::OperationStarted { search_term, .. } => {
            if search_term.starts_with("Pi ") {
                state.activity = Some("Thinking…".into());
            } else {
                state.activity = Some(format!("Searching for `{search_term}`…"));
            }
        }
        Event::Progress {
            files_inspected, ..
        } => state.activity = Some(format!("Inspected {files_inspected} files…")),
        Event::Evidence { id, evidence } => {
            if matches!(
                evidence.source,
                EvidenceSource::Definition | EvidenceSource::LiteralMatch
            ) {
                state
                    .transcript
                    .push(TranscriptItem::Evidence(evidence.clone()));
                state.scroll_from_bottom = 0;
            }
            if state.navigated_for != Some(id) {
                if let Err(cause) = navigate(&evidence) {
                    state.line(format!("Navigation failed: {cause}"));
                }
                state.navigated_for = Some(id);
            }
        }
        Event::ChangeProposal { proposal, .. } => {
            if let Err(message) = show_proposal(selection_path, &proposal) {
                state.line(format!("Preview failed: {message}"));
            }
        }
        Event::TextDelta { id, delta } => {
            state.activity = Some("Responding…".into());
            state.answer_delta(id, &delta);
        }
        Event::ToolStarted {
            tool,
            relative_path,
            ..
        } => {
            let location = relative_path
                .map(|path| format!(" `{}`", path.display()))
                .unwrap_or_default();
            state.activity = Some(format!("{}{}…", tool.label(), location));
        }
        Event::ToolFinished {
            tool,
            relative_path,
            success,
            ..
        } => {
            if !success {
                state.line(format!("{} failed.", tool.label()));
            }
            state.activity = Some("Thinking…".into());
            if success
                && matches!(
                    tool,
                    lantern_protocol::WorkbenchTool::Edit | lantern_protocol::WorkbenchTool::Write
                )
                && let Some(relative_path) = relative_path
            {
                let changed = Evidence {
                    source: EvidenceSource::LiteralMatch,
                    relative_path,
                    start_line: 1,
                    start_column: 1,
                    end_line: 1,
                    end_column: 1,
                    excerpt: String::new(),
                };
                if let Err(cause) = navigate(&changed) {
                    state.line(format!("Could not show the changed file: {cause}"));
                }
            }
        }
        Event::Completed { .. } => state.activity = Some("Finishing…".into()),
        Event::Cancelled {
            id,
            cancellation_latency_ms,
        } => {
            state.line(format!(
                "Interrupted operation {id} in {cancellation_latency_ms} ms."
            ));
        }
        Event::Error {
            id,
            message,
            recovery,
        } => {
            if id.is_none() {
                state.daemon_failed(&message, &format!("Recovery: {recovery}"));
                return Ok(());
            }
            let label = id.map_or_else(|| "error".into(), |id| format!("error [{id}]"));
            state.line(format!("{label}: {message}"));
            state.line(format!("Recovery: {recovery}"));
            if id == state.active_id && id != state.accepted_id {
                state.active_id = None;
                state.navigated_for = None;
            }
        }
        Event::Settled { id } => {
            state.activity = None;
            if state.active_id == Some(id) {
                state.active_id = None;
                state.accepted_id = None;
                state.navigated_for = None;
            }
        }
    }
    Ok(())
}

fn close_session(
    daemon_stdin: &Arc<Mutex<BufWriter<ChildStdin>>>,
    daemon_state: DaemonState,
) -> io::Result<()> {
    let shutdown = if daemon_state == DaemonState::Ready {
        send_request(daemon_stdin, &Request::Shutdown)
    } else {
        Ok(())
    };
    let status = Command::new("tmux")
        .arg("kill-session")
        .arg("-t")
        .arg(env::var("LANTERN_SESSION").unwrap_or_default())
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "tmux could not close the Lantern session: {status}"
        )));
    }
    shutdown
}

fn run(repository: PathBuf, daemon_path: PathBuf, selection_path: PathBuf) -> io::Result<()> {
    let mut daemon = Command::new(daemon_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let daemon_stdin = Arc::new(Mutex::new(BufWriter::new(
        daemon.stdin.take().expect("daemon stdin"),
    )));
    let daemon_stdout = daemon.stdout.take().expect("daemon stdout");
    let daemon_stderr = daemon.stderr.take().expect("daemon stderr");
    let diagnostics = Arc::new(Mutex::new(BoundedTail::new(MAX_DIAGNOSTIC_BYTES)));
    let (sender, receiver): (Sender<Input>, Receiver<Input>) = mpsc::channel();
    let _terminal = TerminalGuard::enter()?;
    spawn_terminal_reader(sender.clone());
    spawn_diagnostic_reader(daemon_stderr, diagnostics.clone());
    spawn_daemon_reader(daemon_stdout, diagnostics.clone(), sender.clone());
    spawn_startup_deadline(sender);
    send_request(
        &daemon_stdin,
        &Request::Initialize {
            protocol_version: PROTOCOL_VERSION,
        },
    )?;
    send_request(
        &daemon_stdin,
        &Request::OpenWorkbench {
            repository: repository.clone(),
        },
    )?;

    let mut state = UiState::new(&repository);
    let mut layout = render(&state)?;
    while let Ok(input) = receiver.recv() {
        let should_quit = match input {
            Input::Terminal(TerminalEvent::Key(key)) => handle_key(
                key,
                &mut state,
                &repository,
                &selection_path,
                &daemon_stdin,
                &diagnostics,
            )?,
            Input::Terminal(TerminalEvent::Mouse(mouse)) => {
                handle_mouse(mouse, &layout, &mut state)?
            }
            Input::Terminal(TerminalEvent::Resize(_, _)) => false,
            Input::Terminal(_) => false,
            Input::Daemon(event) => {
                handle_daemon_event(event, &mut state, &selection_path)?;
                if state.daemon == DaemonState::Unavailable {
                    let _ = daemon.kill();
                }
                false
            }
            Input::DaemonClosed { diagnostics } => {
                state.daemon_failed("the daemon process exited", &diagnostics);
                false
            }
            Input::DaemonStartupTimeout => {
                if state.daemon == DaemonState::Starting {
                    state.daemon_failed(
                        "startup did not complete within two seconds",
                        "Check the daemon binary and restart the Lantern session.",
                    );
                    let _ = daemon.kill();
                }
                false
            }
        };
        if should_quit {
            close_session(&daemon_stdin, state.daemon)?;
            break;
        }
        layout = render(&state)?;
    }

    let _ = daemon.wait();
    Ok(())
}

fn main() -> io::Result<()> {
    let repository = env::var_os("LANTERN_REPO")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("LANTERN_REPO is not configured"))?;
    let daemon = env::var_os("LANTERN_DAEMON_BIN")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("LANTERN_DAEMON_BIN is not configured"))?;
    let selection_path = env::var_os("LANTERN_SELECTION_PATH")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("LANTERN_SELECTION_PATH is not configured"))?;
    run(repository, daemon, selection_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(source: EvidenceSource, path: &str) -> Evidence {
        Evidence {
            source,
            relative_path: path.into(),
            start_line: 4,
            start_column: 2,
            end_line: 4,
            end_column: 8,
            excerpt: "private source body".into(),
        }
    }

    #[test]
    fn resting_state_has_no_persistent_status_messages() {
        let state = UiState::new(Path::new("."));
        assert!(state.transcript.is_empty());
        assert!(state.activity.is_none());
    }

    #[test]
    fn model_control_sequences_are_removed_before_rendering() {
        assert_eq!(clean_text("safe\u{1b}[2J text\nnext"), "safe[2J text\nnext");
    }

    #[test]
    fn diagnostic_summary_excludes_unstructured_process_output() {
        let summary = diagnostic_summary("token=private-value");
        assert!(summary.contains("unstructured line(s) excluded"));
        assert!(!summary.contains("private-value"));
    }

    #[test]
    fn diagnostic_export_is_opt_in_metadata_only() {
        let directory = env::temp_dir().join(format!(
            "lantern-terminal-diagnostics-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&directory).expect("create diagnostic fixture");
        let record = lantern_diagnostics::Record::new(
            lantern_diagnostics::Level::Info,
            lantern_diagnostics::Component::Daemon,
            lantern_diagnostics::Code::DaemonStarted,
        );
        let stderr = format!(
            "password=private-value\n{}\n",
            serde_json::to_string(&record).expect("serialize record")
        );

        let path = write_diagnostic_bundle(&directory, DaemonState::Ready, &stderr)
            .expect("write diagnostic bundle");
        let contents = fs::read_to_string(&path).expect("read diagnostic bundle");
        let bundle: lantern_diagnostics::Bundle =
            serde_json::from_str(&contents).expect("decode diagnostic bundle");
        assert_eq!(bundle.records, [record]);
        assert_eq!(bundle.ignored_unstructured_lines, 1);
        assert!(!contents.contains("private-value"));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path)
                .expect("diagnostic metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_dir_all(directory).expect("remove diagnostic fixture");
    }

    #[test]
    fn wrapping_preserves_explicit_blank_lines() {
        assert_eq!(wrap_text("abcd\n\nef", 2), ["ab", "cd", "", "ef"]);
    }

    #[test]
    fn evidence_rows_explain_typed_provenance_without_rendering_source() {
        let mut state = UiState::new(Path::new("."));
        state.transcript = vec![
            TranscriptItem::Evidence(evidence(EvidenceSource::Selection, "src/main.rs")),
            TranscriptItem::Evidence(evidence(EvidenceSource::Definition, "src/lib.rs")),
        ];

        let rendered = flattened_transcript(&state, 200)
            .into_iter()
            .map(|row| row.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Selected code · exact code highlighted in Helix"));
        assert!(rendered.contains("Definition · symbol definition resolved by Helix"));
        assert!(!rendered.contains("private source body"));
    }

    #[test]
    fn evidence_selection_cycles_without_scanning_or_model_work() {
        let mut state = UiState::new(Path::new("."));
        state.transcript = vec![
            TranscriptItem::Evidence(evidence(EvidenceSource::Selection, "src/main.rs")),
            TranscriptItem::Evidence(evidence(EvidenceSource::Definition, "src/lib.rs")),
        ];

        state.select_evidence(1);
        assert_eq!(state.selected_evidence, Some(0));
        assert_eq!(
            state.evidence(0).map(|item| item.source),
            Some(EvidenceSource::Selection)
        );
        state.select_evidence(1);
        assert_eq!(state.selected_evidence, Some(1));
        state.select_evidence(1);
        assert_eq!(state.selected_evidence, Some(0));
        state.select_evidence(-1);
        assert_eq!(state.selected_evidence, Some(1));
    }

    #[test]
    fn ctrl_d_quits_only_from_an_idle_empty_prompt() {
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert!(is_quit_shortcut(ctrl_d, false, true));
        assert!(!is_quit_shortcut(ctrl_d, true, true));
        assert!(!is_quit_shortcut(ctrl_d, false, false));
    }

    #[test]
    fn an_operation_is_reserved_before_a_second_submit_can_run() {
        let mut state = UiState::new(Path::new("."));
        state.daemon = DaemonState::Ready;
        assert_eq!(state.begin_operation(), Some(1));
        assert_eq!(state.begin_operation(), None);
        assert_eq!(state.active_id, Some(1));
        assert_eq!(state.next_id, 2);
    }

    #[test]
    fn daemon_failure_keeps_the_pane_available_and_clears_operation_state() {
        let mut state = UiState::new(Path::new("."));
        state.daemon = DaemonState::Ready;
        state.active_id = Some(7);
        state.accepted_id = Some(7);
        state.daemon_failed("process exited", "last diagnostic");

        assert_eq!(state.daemon, DaemonState::Unavailable);
        assert_eq!(state.active_id, None);
        assert_eq!(state.accepted_id, None);
        assert!(state.transcript.iter().any(
            |item| matches!(item, TranscriptItem::Line(line) if line.contains("unstructured line(s) excluded"))
        ));
        assert!(!state.transcript.iter().any(
            |item| matches!(item, TranscriptItem::Line(line) if line.contains("last diagnostic"))
        ));
    }

    #[test]
    fn initialization_is_the_explicit_ready_boundary() {
        let mut state = UiState::new(Path::new("."));
        assert_eq!(state.daemon, DaemonState::Starting);
        handle_daemon_event(
            Event::Initialized {
                protocol_version: PROTOCOL_VERSION,
            },
            &mut state,
            Path::new("unused"),
        )
        .expect("handle initialization");
        assert_eq!(state.daemon, DaemonState::Ready);
    }

    #[test]
    fn workbench_open_event_returns_to_the_quiet_prompt() {
        let mut state = UiState::new(Path::new("."));
        state.activity = Some("Opening workbench…".into());
        handle_daemon_event(
            Event::WorkbenchOpened {
                repository: PathBuf::from("/workspace/project"),
            },
            &mut state,
            Path::new("unused"),
        )
        .expect("handle workspace configuration");

        assert!(state.transcript.is_empty());
        assert!(state.activity.is_none());
    }
}
