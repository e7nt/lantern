use lantern_protocol::{
    BoundedTail, ChangeProposal, Event, Evidence, FrameError, MAX_DIAGNOSTIC_BYTES,
    MAX_EVENT_BYTES, MAX_EVIDENCE, MAX_FILE_BYTES, MAX_FILES, MAX_SELECTION_BYTES,
    PROTOCOL_VERSION, Request, SelectionContext, SymbolContext, SymbolLocation, read_frame,
    validate_selection, validate_symbol_context,
};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type SharedWriter = Arc<Mutex<BufWriter<io::Stdout>>>;
type Operations = Arc<Mutex<HashMap<u64, Arc<Cancellation>>>>;

#[derive(Default)]
struct Cancellation {
    requested: AtomicBool,
    requested_at: Mutex<Option<Instant>>,
    abort_stdin: Mutex<Option<ChildStdin>>,
}

struct ChildGuard {
    process: Child,
    reaped: bool,
}

impl ChildGuard {
    fn new(process: Child) -> Self {
        Self {
            process,
            reaped: false,
        }
    }

    fn stop(&mut self) {
        if self.reaped {
            return;
        }
        let _ = self.process.kill();
        let _ = self.process.wait();
        self.reaped = true;
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Cancellation {
    fn request(&self) {
        *self.requested_at.lock().expect("cancellation time lock") = Some(Instant::now());
        self.requested.store(true, Ordering::Release);
        if let Some(stdin) = self.abort_stdin.lock().expect("abort stdin lock").as_mut() {
            let _ = stdin.write_all(b"{\"type\":\"abort\"}\n");
            let _ = stdin.flush();
        }
    }

    fn attach_abort(&self, stdin: ChildStdin) {
        let mut abort_stdin = self.abort_stdin.lock().expect("abort stdin lock");
        *abort_stdin = Some(stdin);
        if self.requested.load(Ordering::Acquire)
            && let Some(stdin) = abort_stdin.as_mut()
        {
            let _ = stdin.write_all(b"{\"type\":\"abort\"}\n");
            let _ = stdin.flush();
        }
    }

    fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    fn latency_ms(&self) -> u64 {
        self.requested_at
            .lock()
            .expect("cancellation time lock")
            .map(|requested_at| requested_at.elapsed().as_millis())
            .unwrap_or_default()
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

fn emit(writer: &SharedWriter, event: &Event) -> io::Result<()> {
    let frame = encode_event(event)?;
    let mut writer = writer.lock().expect("stdout lock");
    writer.write_all(&frame)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn encode_event(event: &Event) -> io::Result<Vec<u8>> {
    let frame = serde_json::to_vec(event).map_err(io::Error::other)?;
    if frame.len() > MAX_EVENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("protocol event exceeds the {MAX_EVENT_BYTES}-byte limit"),
        ));
    }
    Ok(frame)
}

fn error(writer: &SharedWriter, id: Option<u64>, message: impl Into<String>, recovery: &str) {
    let _ = emit(
        writer,
        &Event::Error {
            id,
            message: message.into(),
            recovery: recovery.into(),
        },
    );
}

fn admit(id: u64, operations: &Operations, writer: &SharedWriter) -> Option<Arc<Cancellation>> {
    let cancellation = Arc::new(Cancellation::default());
    let mut active = operations.lock().expect("operations lock");
    if active.contains_key(&id) {
        drop(active);
        error(
            writer,
            Some(id),
            "operation identifier is already active",
            "wait for the active operation or cancel it",
        );
        return None;
    }
    if !active.is_empty() {
        drop(active);
        error(
            writer,
            Some(id),
            "another operation is active",
            "wait for the active operation to settle or cancel it",
        );
        return None;
    }
    active.insert(id, cancellation.clone());
    drop(active);
    if emit(writer, &Event::Accepted { id }).is_err() {
        operations.lock().expect("operations lock").remove(&id);
        return None;
    }
    Some(cancellation)
}

fn settle(id: u64, operations: &Operations, writer: &SharedWriter) {
    operations.lock().expect("operations lock").remove(&id);
    let _ = emit(writer, &Event::Settled { id });
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".lantern" | "node_modules" | "target"))
}

fn evidence_for(
    path: &Path,
    relative_path: PathBuf,
    contents: &str,
    query: &str,
) -> Option<Evidence> {
    let byte_index = contents.find(query)?;
    let before = &contents[..byte_index];
    let start_line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let start_column = contents[line_start..byte_index].chars().count() + 1;
    let end_column = start_column + query.chars().count();
    let excerpt = contents[byte_index..]
        .lines()
        .next()
        .unwrap_or(query)
        .chars()
        .take(160)
        .collect();

    debug_assert!(path.is_file());
    Some(Evidence {
        relative_path,
        start_line,
        start_column,
        end_line: start_line,
        end_column,
        excerpt,
    })
}

fn evidence_from_symbol_location(
    root: &Path,
    location: &SymbolLocation,
    excerpt_lines: usize,
) -> Result<Evidence, String> {
    let path = root.join(&location.relative_path);
    let canonical_path = path
        .canonicalize()
        .map_err(|cause| format!("cannot open LSP evidence {}: {cause}", path.display()))?;
    if !canonical_path.starts_with(root) {
        return Err("LSP evidence escaped the repository".into());
    }
    let metadata = canonical_path
        .metadata()
        .map_err(|cause| format!("cannot inspect LSP evidence {}: {cause}", path.display()))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "LSP evidence {} exceeds the {MAX_FILE_BYTES}-byte limit",
            location.relative_path.display()
        ));
    }
    let contents = fs::read_to_string(&canonical_path)
        .map_err(|cause| format!("cannot read LSP evidence {}: {cause}", path.display()))?;
    let mut lines = contents.lines().skip(location.start_line - 1);
    let line = lines
        .next()
        .ok_or_else(|| format!("LSP evidence line is outside {}", path.display()))?;
    if location.start_column > line.chars().count() + 1 {
        return Err(format!("LSP evidence column is outside {}", path.display()));
    }
    let excerpt = std::iter::once(line)
        .chain(lines.take(excerpt_lines.saturating_sub(1)))
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(1_024)
        .collect();
    Ok(Evidence {
        relative_path: location.relative_path.clone(),
        start_line: location.start_line,
        start_column: location.start_column,
        end_line: location.end_line,
        end_column: location.end_column,
        excerpt,
    })
}

fn collect_evidence(
    root: &Path,
    query: &str,
    cancellation: &Cancellation,
    writer: &SharedWriter,
    id: u64,
) -> Result<Vec<Evidence>, String> {
    let mut pending = vec![root.to_owned()];
    let mut inspected = 0;
    let mut evidence = Vec::new();

    while let Some(directory) = pending.pop() {
        if cancellation.is_requested() || inspected >= MAX_FILES || evidence.len() >= MAX_EVIDENCE {
            break;
        }

        let entries = fs::read_dir(&directory)
            .map_err(|cause| format!("cannot read {}: {cause}", directory.display()))?;
        for entry in entries {
            if cancellation.is_requested()
                || inspected >= MAX_FILES
                || evidence.len() >= MAX_EVIDENCE
            {
                break;
            }

            let entry = entry.map_err(|cause| format!("cannot read directory entry: {cause}"))?;
            let path = entry.path();
            if should_skip(&path) {
                continue;
            }
            let file_type = entry
                .file_type()
                .map_err(|cause| format!("cannot inspect {}: {cause}", path.display()))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let metadata = entry
                .metadata()
                .map_err(|cause| format!("cannot inspect {}: {cause}", path.display()))?;
            if metadata.len() > MAX_FILE_BYTES {
                continue;
            }

            inspected += 1;
            if inspected == 1 || inspected % 25 == 0 {
                emit(
                    writer,
                    &Event::Progress {
                        id,
                        files_inspected: inspected,
                    },
                )
                .map_err(|cause| format!("cannot stream progress: {cause}"))?;
            }

            let bytes = fs::read(&path)
                .map_err(|cause| format!("cannot read {}: {cause}", path.display()))?;
            if bytes.contains(&0) {
                continue;
            }
            let Ok(contents) = String::from_utf8(bytes) else {
                continue;
            };
            let relative_path = path
                .strip_prefix(root)
                .map_err(|_| format!("{} escaped the repository", path.display()))?
                .to_owned();
            if let Some(found) = evidence_for(&path, relative_path, &contents, query) {
                emit(
                    writer,
                    &Event::Evidence {
                        id,
                        evidence: found.clone(),
                    },
                )
                .map_err(|cause| format!("cannot stream evidence: {cause}"))?;
                evidence.push(found);
            }
        }
    }

    Ok(evidence)
}

fn stream_answer(
    id: u64,
    query: &str,
    evidence: &[Evidence],
    cancellation: &Cancellation,
    writer: &SharedWriter,
) -> io::Result<()> {
    let answer = if let Some(first) = evidence.first() {
        format!(
            "Found {} exact repository match{}. The strongest evidence is {}:{} and is now selected in Helix. This first slice uses literal local evidence; symbol and LSP reasoning are not enabled yet.",
            evidence.len(),
            if evidence.len() == 1 { "" } else { "es" },
            first.relative_path.display(),
            first.start_line,
        )
    } else {
        format!(
            "No exact repository match was found for `{query}` within the bounded local search. Try a symbol or shorter literal; Lantern did not substitute a semantic or model-based search."
        )
    };

    for word in answer.split_inclusive(' ') {
        if cancellation.is_requested() {
            return Ok(());
        }
        emit(
            writer,
            &Event::TextDelta {
                id,
                delta: word.to_owned(),
            },
        )?;
        thread::sleep(Duration::from_millis(35));
    }
    Ok(())
}

fn run_operation(
    id: u64,
    repository: PathBuf,
    query: String,
    cancellation: Arc<Cancellation>,
    operations: Operations,
    writer: SharedWriter,
) {
    let result = (|| -> Result<usize, String> {
        let root = repository
            .canonicalize()
            .map_err(|cause| format!("cannot open repository {}: {cause}", repository.display()))?;
        if !root.is_dir() {
            return Err(format!("repository is not a directory: {}", root.display()));
        }

        emit(
            &writer,
            &Event::OperationStarted {
                id,
                search_term: query.clone(),
            },
        )
        .map_err(|cause| format!("cannot stream operation start: {cause}"))?;
        let evidence = collect_evidence(&root, &query, &cancellation, &writer, id)?;
        if !cancellation.is_requested() {
            stream_answer(id, &query, &evidence, &cancellation, &writer)
                .map_err(|cause| format!("cannot stream answer: {cause}"))?;
        }
        Ok(evidence.len())
    })();

    if cancellation.is_requested() {
        let _ = emit(
            &writer,
            &Event::Cancelled {
                id,
                cancellation_latency_ms: cancellation.latency_ms(),
            },
        );
    } else {
        match result {
            Ok(evidence_count) => {
                let _ = emit(&writer, &Event::Completed { id, evidence_count });
            }
            Err(message) => error(
                &writer,
                Some(id),
                message,
                "check repository permissions and retry the explicit search",
            ),
        }
    }
    settle(id, &operations, &writer);
}

fn run_selection_operation(
    id: u64,
    repository: PathBuf,
    query: String,
    selection: SelectionContext,
    cancellation: Arc<Cancellation>,
    operations: Operations,
    writer: SharedWriter,
) {
    let result = (|| -> Result<(), String> {
        validate_selection(&selection)?;
        let root = repository
            .canonicalize()
            .map_err(|cause| format!("cannot open repository {}: {cause}", repository.display()))?;
        let selected_path = root.join(&selection.relative_path);
        let canonical_path = selected_path.canonicalize().map_err(|cause| {
            format!(
                "cannot open selected file {}: {cause}",
                selected_path.display()
            )
        })?;
        if !canonical_path.starts_with(&root) {
            return Err("selected file escaped the repository".into());
        }

        emit(
            &writer,
            &Event::OperationStarted {
                id,
                search_term: query,
            },
        )
        .map_err(|cause| format!("cannot stream operation start: {cause}"))?;
        let evidence = Evidence {
            relative_path: selection.relative_path.clone(),
            start_line: selection.start_line,
            start_column: selection.start_column,
            end_line: selection.end_line,
            end_column: selection.end_column,
            excerpt: selection
                .text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect(),
        };
        emit(&writer, &Event::Evidence { id, evidence })
            .map_err(|cause| format!("cannot stream selection evidence: {cause}"))?;

        let answer = format!(
            "Captured {} selected character{} from {}{} through the typed editor boundary. No model or symbol reasoning was used in this gate.",
            selection.text.chars().count(),
            if selection.text.chars().count() == 1 {
                ""
            } else {
                "s"
            },
            selection.relative_path.display(),
            if selection.document_modified {
                " (including unsaved buffer changes)"
            } else {
                ""
            },
        );
        for word in answer.split_inclusive(' ') {
            if cancellation.is_requested() {
                return Ok(());
            }
            emit(
                &writer,
                &Event::TextDelta {
                    id,
                    delta: word.to_owned(),
                },
            )
            .map_err(|cause| format!("cannot stream answer: {cause}"))?;
            thread::sleep(Duration::from_millis(35));
        }
        Ok(())
    })();

    if cancellation.is_requested() {
        let _ = emit(
            &writer,
            &Event::Cancelled {
                id,
                cancellation_latency_ms: cancellation.latency_ms(),
            },
        );
    } else if let Err(message) = result {
        error(
            &writer,
            Some(id),
            message,
            "select saved repository text in Helix and retry `/ask <question>`",
        );
    } else {
        let _ = emit(
            &writer,
            &Event::Completed {
                id,
                evidence_count: 1,
            },
        );
    }
    settle(id, &operations, &writer);
}

enum AgentContext {
    Selection(SelectionContext),
    Symbol(SymbolContext),
}

fn run_pi_operation(
    id: u64,
    repository: PathBuf,
    query: String,
    agent_context: AgentContext,
    cancellation: Arc<Cancellation>,
    operations: Operations,
    writer: SharedWriter,
) {
    let (selection, symbol_context) = match agent_context {
        AgentContext::Selection(selection) => (selection, None),
        AgentContext::Symbol(context) => (context.selection.clone(), Some(context)),
    };
    let result = (|| -> Result<(), String> {
        validate_selection(&selection)?;
        if let Some(context) = &symbol_context {
            validate_symbol_context(context)?;
            if context.selection != selection {
                return Err("symbol context selection does not match the agent selection".into());
            }
        }
        let root = repository
            .canonicalize()
            .map_err(|cause| format!("cannot open repository {}: {cause}", repository.display()))?;
        let selected_path = root.join(&selection.relative_path);
        let canonical_path = selected_path.canonicalize().map_err(|cause| {
            format!(
                "cannot open selected file {}: {cause}",
                selected_path.display()
            )
        })?;
        if !canonical_path.starts_with(&root) {
            return Err("selected file escaped the repository".into());
        }

        let pi_bin =
            std::env::var_os("LANTERN_PI_BIN").ok_or("LANTERN_PI_BIN is not configured")?;
        let model =
            std::env::var("LANTERN_PI_MODEL").map_err(|_| "LANTERN_PI_MODEL is not configured")?;
        let workdir = std::env::var_os("LANTERN_MODEL_WORKDIR")
            .ok_or("LANTERN_MODEL_WORKDIR is not configured")?;
        emit(
            &writer,
            &Event::OperationStarted {
                id,
                search_term: format!("Pi {model}"),
            },
        )
        .map_err(|cause| format!("cannot stream operation start: {cause}"))?;

        let selection_evidence = Evidence {
            relative_path: selection.relative_path.clone(),
            start_line: selection.start_line,
            start_column: selection.start_column,
            end_line: selection.end_line,
            end_column: selection.end_column,
            excerpt: selection
                .text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(160)
                .collect(),
        };
        emit(
            &writer,
            &Event::Evidence {
                id,
                evidence: selection_evidence,
            },
        )
        .map_err(|cause| format!("cannot stream selected evidence: {cause}"))?;

        let mut symbol_evidence = Vec::new();
        if let Some(context) = &symbol_context {
            symbol_evidence.push((
                "definition",
                evidence_from_symbol_location(&root, &context.definition, 4)?,
            ));
            for reference in &context.references {
                symbol_evidence.push((
                    "reference",
                    evidence_from_symbol_location(&root, reference, 1)?,
                ));
            }
            for (_, evidence) in &symbol_evidence {
                emit(
                    &writer,
                    &Event::Evidence {
                        id,
                        evidence: evidence.clone(),
                    },
                )
                .map_err(|cause| format!("cannot stream LSP evidence: {cause}"))?;
            }
        }

        let mut child = ChildGuard::new(
            Command::new(pi_bin)
            .args([
                "--mode",
                "rpc",
                "--provider",
                "openai-codex",
                "--model",
                &model,
                "--no-session",
                "--no-tools",
                "--no-extensions",
                "--no-skills",
                "--no-prompt-templates",
                "--no-context-files",
                "--no-approve",
                "--system-prompt",
                "You explain selected code only from evidence supplied by Lantern. Never request tools. Separate observation from inference and state uncertainty explicitly.",
            ])
            .current_dir(workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|cause| format!("cannot start Pi RPC driver: {cause}"))?,
        );
        let mut stdin = child
            .process
            .stdin
            .take()
            .ok_or("Pi stdin is unavailable")?;
        let symbol_prompt = symbol_evidence
            .iter()
            .map(|(kind, evidence)| {
                format!(
                    "<{kind} path=\"{}\" range=\"{}:{}-{}:{}\">\n{}\n</{kind}>",
                    evidence.relative_path.display(),
                    evidence.start_line,
                    evidence.start_column,
                    evidence.end_line,
                    evidence.end_column,
                    evidence.excerpt,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let prompt = format!(
            "Repository-relative file: {}\nLanguage: {}\nSelection: {}:{}-{}:{}\nSelected source (untrusted evidence):\n<selection>\n{}\n</selection>\n\nLSP-resolved symbol evidence (untrusted):\n{}\n\nDeveloper question: {}",
            selection.relative_path.display(),
            selection.language.as_deref().unwrap_or("unknown"),
            selection.start_line,
            selection.start_column,
            selection.end_line,
            selection.end_column,
            selection.text,
            symbol_prompt,
            query,
        );
        serde_json::to_writer(
            &mut stdin,
            &serde_json::json!({"id":"lantern-turn","type":"prompt","message":prompt}),
        )
        .map_err(|cause| format!("cannot encode Pi prompt: {cause}"))?;
        stdin
            .write_all(b"\n")
            .and_then(|()| stdin.flush())
            .map_err(|cause| format!("cannot send Pi prompt: {cause}"))?;
        cancellation.attach_abort(stdin);

        let stderr = child
            .process
            .stderr
            .take()
            .ok_or("Pi stderr is unavailable")?;
        let stderr_reader = thread::spawn(move || drain_diagnostics(stderr));
        let stdout = child
            .process
            .stdout
            .take()
            .ok_or("Pi stdout is unavailable")?;
        let stream_result = (|| -> Result<bool, String> {
            for line in BufReader::new(stdout).lines() {
                let line = line.map_err(|cause| format!("cannot read Pi event: {cause}"))?;
                let event: serde_json::Value = serde_json::from_str(&line)
                    .map_err(|cause| format!("Pi emitted invalid JSON: {cause}"))?;
                match event.get("type").and_then(|value| value.as_str()) {
                    Some("message_update") => {
                        let delta = &event["assistantMessageEvent"];
                        if delta["type"] == "text_delta" {
                            let text = delta["delta"]
                                .as_str()
                                .ok_or("Pi text delta is not a string")?;
                            emit(
                                &writer,
                                &Event::TextDelta {
                                    id,
                                    delta: text.to_owned(),
                                },
                            )
                            .map_err(|cause| format!("cannot stream Pi text: {cause}"))?;
                        }
                    }
                    Some("agent_settled") => return Ok(true),
                    Some("response") if event["success"] == false => {
                        return Err(format!(
                            "Pi rejected the request: {}",
                            event["error"].as_str().unwrap_or("unknown error")
                        ));
                    }
                    Some("tool_execution_start") => {
                        cancellation.request();
                        return Err("Pi requested a tool despite the no-tools boundary".into());
                    }
                    _ => {}
                }
            }
            Ok(false)
        })();
        child.stop();
        let stderr = stderr_reader.join().unwrap_or_default();
        let saw_agent_settled = stream_result?;
        if !saw_agent_settled && !cancellation.is_requested() {
            return Err(if stderr.trim().is_empty() {
                "Pi closed before the agent turn settled".into()
            } else {
                format!("Pi closed before the agent turn settled: {}", stderr.trim())
            });
        }
        Ok(())
    })();

    if cancellation.is_requested() {
        let _ = emit(
            &writer,
            &Event::Cancelled {
                id,
                cancellation_latency_ms: cancellation.latency_ms(),
            },
        );
    } else if let Err(message) = result {
        error(
            &writer,
            Some(id),
            message,
            "run Pi interactively, use `/login` for OpenAI Codex, then retry `/agent <question>`",
        );
    } else {
        let _ = emit(
            &writer,
            &Event::Completed {
                id,
                evidence_count: 1 + symbol_context
                    .as_ref()
                    .map_or(0, |context| 1 + context.references.len()),
            },
        );
    }
    settle(id, &operations, &writer);
}

fn drain_diagnostics(mut reader: impl Read) -> String {
    let mut tail = BoundedTail::new(MAX_DIAGNOSTIC_BYTES);
    let mut chunk = [0_u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => tail.push(&chunk[..read]),
            Err(_) => break,
        }
    }
    tail.text()
}

fn main() -> io::Result<()> {
    let writer = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    let operations: Operations = Arc::new(Mutex::new(HashMap::new()));
    let mut workers = Vec::new();
    let mut input = BufReader::new(io::stdin());
    let mut initialized = false;

    loop {
        let line = match read_frame(&mut input) {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(FrameError::Io(cause)) => return Err(cause),
            Err(cause) => {
                error(
                    &writer,
                    None,
                    cause.to_string(),
                    "send one LF-delimited UTF-8 JSON request within the frame limit",
                );
                continue;
            }
        };
        let request = match serde_json::from_str::<Request>(&line) {
            Ok(request) => request,
            Err(cause) => {
                error(
                    &writer,
                    None,
                    format!("invalid protocol request: {cause}"),
                    "send one valid JSON request per line",
                );
                continue;
            }
        };

        if !initialized && !matches!(&request, Request::Initialize { .. } | Request::Shutdown) {
            error(
                &writer,
                None,
                "daemon is not initialized",
                "initialize with the exact supported protocol version before sending work",
            );
            continue;
        }

        match request {
            Request::Initialize { protocol_version } => {
                if protocol_version != PROTOCOL_VERSION {
                    error(
                        &writer,
                        None,
                        format!(
                            "protocol version {protocol_version} is unsupported; expected {PROTOCOL_VERSION}"
                        ),
                        "rebuild the Lantern spike client and daemon from the same checkout",
                    );
                } else {
                    initialized = true;
                    emit(
                        &writer,
                        &Event::Initialized {
                            protocol_version: PROTOCOL_VERSION,
                        },
                    )?;
                }
            }
            Request::Ask {
                id,
                repository,
                query,
            } => {
                if query.trim().is_empty() {
                    error(
                        &writer,
                        Some(id),
                        "query is empty",
                        "enter `/show <literal text>`",
                    );
                    continue;
                }
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_operation(
                        id,
                        repository,
                        query,
                        cancellation,
                        active_operations,
                        operation_writer,
                    );
                }));
            }
            Request::AskSelection {
                id,
                repository,
                query,
                selection,
            } => {
                if query.trim().is_empty() {
                    error(
                        &writer,
                        Some(id),
                        "query is empty",
                        "enter `/ask <question>`",
                    );
                    continue;
                }
                if let Err(message) = validate_selection(&selection) {
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select saved repository text in Helix and retry `/ask <question>`",
                    );
                    continue;
                }
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_selection_operation(
                        id,
                        repository,
                        query,
                        selection,
                        cancellation,
                        active_operations,
                        operation_writer,
                    );
                }));
            }
            Request::AskAgentSelection {
                id,
                repository,
                query,
                selection,
            } => {
                if query.trim().is_empty() {
                    error(
                        &writer,
                        Some(id),
                        "query is empty",
                        "enter `/agent <question>`",
                    );
                    continue;
                }
                if let Err(message) = validate_selection(&selection) {
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select saved repository text in Helix and retry the question",
                    );
                    continue;
                }
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_pi_operation(
                        id,
                        repository,
                        query,
                        AgentContext::Selection(selection),
                        cancellation,
                        active_operations,
                        operation_writer,
                    );
                }));
            }
            Request::AskAgentSymbol {
                id,
                repository,
                query,
                context,
            } => {
                if query.trim().is_empty() {
                    error(
                        &writer,
                        Some(id),
                        "query is empty",
                        "select a symbol and enter a question",
                    );
                    continue;
                }
                if let Err(message) = validate_symbol_context(&context) {
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select a symbol that Helix can resolve and retry the question",
                    );
                    continue;
                }
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_pi_operation(
                        id,
                        repository,
                        query,
                        AgentContext::Symbol(context),
                        cancellation,
                        active_operations,
                        operation_writer,
                    );
                }));
            }
            Request::PreviewSelection {
                id,
                repository,
                selection,
                replacement,
            } => {
                if let Err(message) = validate_selection(&selection) {
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select repository text and provide `/preview <one-line replacement>`",
                    );
                    continue;
                }
                if replacement.is_empty() || replacement.len() > MAX_SELECTION_BYTES {
                    let message = if replacement.is_empty() {
                        "replacement is empty".into()
                    } else {
                        format!("replacement exceeds the {MAX_SELECTION_BYTES}-byte limit")
                    };
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select repository text and provide `/preview <one-line replacement>`",
                    );
                    continue;
                }
                let Some(_cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let result = (|| -> Result<(), String> {
                    let root = repository.canonicalize().map_err(|cause| {
                        format!("cannot open repository {}: {cause}", repository.display())
                    })?;
                    let selected_path = root.join(&selection.relative_path);
                    let canonical_path = selected_path.canonicalize().map_err(|cause| {
                        format!(
                            "cannot open selected file {}: {cause}",
                            selected_path.display()
                        )
                    })?;
                    if !canonical_path.starts_with(&root) {
                        return Err("selected file escaped the repository".into());
                    }
                    emit(
                        &writer,
                        &Event::OperationStarted {
                            id,
                            search_term: "change preview".into(),
                        },
                    )
                    .map_err(|cause| format!("cannot stream operation start: {cause}"))?;
                    emit(
                        &writer,
                        &Event::ChangeProposal {
                            id,
                            proposal: ChangeProposal {
                                selection,
                                replacement,
                            },
                        },
                    )
                    .map_err(|cause| format!("cannot stream change proposal: {cause}"))?;
                    emit(
                        &writer,
                        &Event::Completed {
                            id,
                            evidence_count: 1,
                        },
                    )
                    .map_err(|cause| format!("cannot complete change preview: {cause}"))
                })();
                if let Err(message) = result {
                    error(
                        &writer,
                        Some(id),
                        message,
                        "select repository text and provide `/preview <one-line replacement>`",
                    );
                }
                settle(id, &operations, &writer);
            }
            Request::Cancel { id } => {
                if let Some(operation) = operations.lock().expect("operations lock").get(&id) {
                    operation.request();
                }
            }
            Request::Shutdown => {
                for operation in operations.lock().expect("operations lock").values() {
                    operation.request();
                }
                break;
            }
        }
    }

    for operation in operations.lock().expect("operations lock").values() {
        operation.request();
    }
    for worker in workers {
        let _ = worker.join();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_encoding_rejects_an_unbounded_model_delta() {
        let event = Event::TextDelta {
            id: 1,
            delta: "x".repeat(MAX_EVENT_BYTES),
        };
        let error = encode_event(&event).expect_err("oversized event must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn diagnostic_drain_keeps_reading_after_the_tail_is_full() {
        let diagnostics = [vec![b'x'; MAX_DIAGNOSTIC_BYTES * 4], b"tail".to_vec()].concat();
        let tail = drain_diagnostics(diagnostics.as_slice());
        assert_eq!(tail.len(), MAX_DIAGNOSTIC_BYTES);
        assert!(tail.ends_with("tail"));
    }
}
