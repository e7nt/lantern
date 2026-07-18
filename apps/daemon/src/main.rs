use lantern_diagnostics::{Code as DiagnosticCode, Component, Level, Record, emit as diagnose};
use lantern_protocol::{
    ChangeProposal, Event, Evidence, EvidenceSource, FrameError, MAX_EVENT_BYTES, MAX_EVIDENCE,
    MAX_FILE_BYTES, MAX_FILES, MAX_SELECTION_BYTES, PROTOCOL_VERSION, Request, SelectionContext,
    SymbolContext, SymbolLocation, WorkbenchTool, read_frame, validate_relative_path,
    validate_selection, validate_symbol_context,
};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

type SharedWriter = Arc<Mutex<BufWriter<io::Stdout>>>;
type Operations = Arc<Mutex<HashMap<u64, Arc<Cancellation>>>>;
type PiStdin = Arc<Mutex<BufWriter<ChildStdin>>>;
const DEFINITION_EVIDENCE_LINES: usize = 16;
const CALL_EVIDENCE_LINES: usize = 32;
const SELECTION_EVIDENCE_LINES: usize = 3;

#[derive(Default)]
struct Cancellation {
    requested: AtomicBool,
    requested_at: Mutex<Option<Instant>>,
    abort_stdin: Mutex<Option<PiStdin>>,
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

struct PiDriver {
    root: PathBuf,
    model: String,
    stdin: PiStdin,
    stdout: Mutex<BufReader<ChildStdout>>,
    process: Mutex<ChildGuard>,
    stderr_reader: Mutex<Option<thread::JoinHandle<bool>>>,
}

impl PiDriver {
    fn spawn(root: PathBuf) -> Result<Self, String> {
        let pi_bin =
            std::env::var_os("LANTERN_PI_BIN").ok_or("LANTERN_PI_BIN is not configured")?;
        let model =
            std::env::var("LANTERN_PI_MODEL").map_err(|_| "LANTERN_PI_MODEL is not configured")?;
        let mut process = ChildGuard::new(
            Command::new(pi_bin)
                .args([
                "--mode",
                "rpc",
                "--provider",
                "openai-codex",
                "--model",
                &model,
                "--no-session",
                "--tools",
                "read,grep,find,ls,edit,write,bash",
                "--no-extensions",
                "--no-skills",
                "--no-prompt-templates",
                "--no-context-files",
                "--no-approve",
                "--system-prompt",
                "You are Lantern's coding agent inside a trusted repository. Help the developer understand and write code. Lantern may supply source already resolved and inspected by Helix/LSP. When that evidence fully answers the question, answer immediately without tools; search only for a specific missing fact. Otherwise inspect before making claims and use the fewest useful tool calls: do not repeat equivalent discovery, and prefer a targeted read or search over broad exploration. Make focused edits, run the narrowest useful verification, and use Git deliberately. Lantern already shows tool activity, so do not narrate routine tool steps. After the work, give one concise result with verification and any real caveat. Never expose credentials or unrelated private data.",
                ])
                .current_dir(&root)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|cause| format!("cannot start Pi RPC driver: {cause}"))?,
        );
        let stdin = Arc::new(Mutex::new(BufWriter::new(
            process
                .process
                .stdin
                .take()
                .ok_or("Pi stdin is unavailable")?,
        )));
        let stdout = Mutex::new(BufReader::new(
            process
                .process
                .stdout
                .take()
                .ok_or("Pi stdout is unavailable")?,
        ));
        let stderr = process
            .process
            .stderr
            .take()
            .ok_or("Pi stderr is unavailable")?;
        let stderr_reader = thread::spawn(move || drain_provider_stderr(stderr));
        Ok(Self {
            root,
            model,
            stdin,
            stdout,
            process: Mutex::new(process),
            stderr_reader: Mutex::new(Some(stderr_reader)),
        })
    }

    fn send(&self, value: &serde_json::Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().expect("Pi stdin lock");
        serde_json::to_writer(&mut *stdin, value)
            .map_err(|cause| format!("cannot encode Pi command: {cause}"))?;
        stdin
            .write_all(b"\n")
            .and_then(|()| stdin.flush())
            .map_err(|cause| format!("cannot send Pi command: {cause}"))
    }

    fn set_thinking_level(&self, level: &str) -> Result<(), String> {
        self.send(&serde_json::json!({
            "id": format!("lantern-thinking-{level}"),
            "type": "set_thinking_level",
            "level": level,
        }))
    }

    fn stop(&self) {
        self.process.lock().expect("Pi process lock").stop();
        if let Some(reader) = self.stderr_reader.lock().expect("Pi stderr lock").take() {
            let _ = reader.join();
        }
    }
}

impl Drop for PiDriver {
    fn drop(&mut self) {
        self.process.get_mut().expect("Pi process lock").stop();
        if let Some(reader) = self.stderr_reader.get_mut().expect("Pi stderr lock").take() {
            let _ = reader.join();
        }
    }
}

impl Cancellation {
    fn request(&self) {
        *self.requested_at.lock().expect("cancellation time lock") = Some(Instant::now());
        self.requested.store(true, Ordering::Release);
        if let Some(stdin) = self.abort_stdin.lock().expect("abort stdin lock").as_mut() {
            let mut stdin = stdin.lock().expect("Pi stdin lock");
            let _ = stdin.write_all(b"{\"type\":\"abort\"}\n");
            let _ = stdin.flush();
        }
    }

    fn attach_abort(&self, stdin: PiStdin) {
        let mut abort_stdin = self.abort_stdin.lock().expect("abort stdin lock");
        *abort_stdin = Some(stdin);
        if self.requested.load(Ordering::Acquire)
            && let Some(stdin) = abort_stdin.as_mut()
        {
            let mut stdin = stdin.lock().expect("Pi stdin lock");
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
    let record = if let Some(id) = id {
        Record::new(
            Level::Error,
            Component::Daemon,
            DiagnosticCode::RequestFailed,
        )
        .for_operation(id)
    } else {
        Record::new(
            Level::Warning,
            Component::Protocol,
            DiagnosticCode::ProtocolRejected,
        )
    };
    let _ = diagnose(&record);
    let _ = emit(
        writer,
        &Event::Error {
            id,
            message: message.into(),
            recovery: recovery.into(),
        },
    );
}

fn workspace_error(writer: &SharedWriter, message: impl Into<String>, recovery: &str) {
    let _ = diagnose(&Record::new(
        Level::Warning,
        Component::Workbench,
        DiagnosticCode::WorkbenchRejected,
    ));
    let _ = emit(
        writer,
        &Event::WorkbenchOpenFailed {
            message: message.into(),
            recovery: recovery.into(),
        },
    );
}

fn canonical_repository(repository: &Path) -> Result<PathBuf, String> {
    let root = repository
        .canonicalize()
        .map_err(|cause| format!("cannot open repository {}: {cause}", repository.display()))?;
    if !root.is_dir() {
        return Err(format!("repository is not a directory: {}", root.display()));
    }
    Ok(root)
}

fn opened_repository(
    workbench: &Option<PathBuf>,
    repository: &Path,
    writer: &SharedWriter,
    id: u64,
) -> Option<PathBuf> {
    let root = match canonical_repository(repository) {
        Ok(root) => root,
        Err(message) => {
            error(
                writer,
                Some(id),
                message,
                "open an existing repository and retry",
            );
            return None;
        }
    };
    if workbench.as_ref() != Some(&root) {
        error(
            writer,
            Some(id),
            "request does not belong to the open workbench",
            "open the repository as the active workbench and retry",
        );
        return None;
    }
    Some(root)
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
    let _ = diagnose(
        &Record::new(
            Level::Info,
            Component::Daemon,
            DiagnosticCode::OperationAccepted,
        )
        .for_operation(id),
    );
    Some(cancellation)
}

fn settle(id: u64, operations: &Operations, writer: &SharedWriter) {
    operations.lock().expect("operations lock").remove(&id);
    let _ = emit(writer, &Event::Settled { id });
    let _ = diagnose(
        &Record::new(
            Level::Info,
            Component::Daemon,
            DiagnosticCode::OperationSettled,
        )
        .for_operation(id),
    );
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git"
                    | ".lantern"
                    | ".pytest_cache"
                    | ".ruff_cache"
                    | ".venv"
                    | "__pycache__"
                    | "node_modules"
                    | "target"
                    | "venv"
            )
        })
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
        source: EvidenceSource::LiteralMatch,
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
    source: EvidenceSource,
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
        source,
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
            "Found {} exact repository match{}. The first evidence is {}:{} and is now selected in Helix. This first slice uses literal local evidence; symbol and LSP reasoning are not enabled yet.",
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

    if !cancellation.is_requested() {
        emit(writer, &Event::TextDelta { id, delta: answer })?;
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
        let root = canonical_repository(&repository)?;

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
        let root = canonical_repository(&repository)?;
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
            source: EvidenceSource::Selection,
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
        if !cancellation.is_requested() {
            emit(&writer, &Event::TextDelta { id, delta: answer })
                .map_err(|cause| format!("cannot stream answer: {cause}"))?;
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
    Repository,
    Selection(SelectionContext),
    Symbol(SymbolContext),
}

fn run_pi_operation(
    id: u64,
    driver: Arc<PiDriver>,
    query: String,
    agent_context: AgentContext,
    cancellation: Arc<Cancellation>,
    operations: Operations,
    writer: SharedWriter,
) {
    let (selection, symbol_context) = match agent_context {
        AgentContext::Repository => (None, None),
        AgentContext::Selection(selection) => (Some(selection), None),
        AgentContext::Symbol(context) => (Some(context.selection.clone()), Some(context)),
    };
    let result = (|| -> Result<(), String> {
        if let Some(selection) = &selection {
            validate_selection(selection)?;
        }
        if let Some(context) = &symbol_context {
            validate_symbol_context(context)?;
            if Some(&context.selection) != selection.as_ref() {
                return Err("symbol context selection does not match the agent selection".into());
            }
        }
        let root = driver.root.clone();
        if let Some(selection) = &selection {
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
        }

        emit(
            &writer,
            &Event::OperationStarted {
                id,
                search_term: format!("Pi {}", driver.model),
            },
        )
        .map_err(|cause| format!("cannot stream operation start: {cause}"))?;

        let selection_evidence = if let Some(selection) = &selection {
            let evidence = if symbol_context.is_some() && !selection.document_modified {
                evidence_from_symbol_location(
                    &root,
                    &SymbolLocation {
                        relative_path: selection.relative_path.clone(),
                        start_line: selection.start_line,
                        start_column: selection.start_column,
                        end_line: selection.end_line,
                        end_column: selection.end_column,
                    },
                    SELECTION_EVIDENCE_LINES,
                    EvidenceSource::Selection,
                )?
            } else {
                Evidence {
                    source: EvidenceSource::Selection,
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
                }
            };
            emit(
                &writer,
                &Event::Evidence {
                    id,
                    evidence: evidence.clone(),
                },
            )
            .map_err(|cause| format!("cannot stream selected evidence: {cause}"))?;
            Some(evidence)
        } else {
            None
        };

        let mut symbol_evidence = Vec::new();
        let mut call_evidence = Vec::new();
        if let Some(context) = &symbol_context {
            symbol_evidence.push((
                "definition",
                evidence_from_symbol_location(
                    &root,
                    &context.definition,
                    DEFINITION_EVIDENCE_LINES,
                    EvidenceSource::Definition,
                )?,
            ));
            for reference in &context.references {
                symbol_evidence.push((
                    "reference",
                    evidence_from_symbol_location(&root, reference, 1, EvidenceSource::Reference)?,
                ));
            }
            for call in &context.calls {
                call_evidence.push((
                    call.name.as_str(),
                    call.depth,
                    evidence_from_symbol_location(
                        &root,
                        &call.location,
                        CALL_EVIDENCE_LINES,
                        EvidenceSource::Call,
                    )?,
                ));
            }
            for (_, _, evidence) in call_evidence.iter().rev() {
                emit(
                    &writer,
                    &Event::Evidence {
                        id,
                        evidence: evidence.clone(),
                    },
                )
                .map_err(|cause| format!("cannot stream LSP call evidence: {cause}"))?;
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

        let mut symbol_prompt = symbol_evidence
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
        for (name, depth, evidence) in &call_evidence {
            symbol_prompt.push_str(&format!(
                "\n<call path=\"{}\" range=\"{}:{}-{}:{}\">\nName (untrusted): {name:?}\nDepth: {depth}\n{}\n</call>",
                evidence.relative_path.display(),
                evidence.start_line,
                evidence.start_column,
                evidence.end_line,
                evidence.end_column,
                evidence.excerpt,
            ));
        }
        let editor_context = selection.as_ref().map_or_else(
            || "No editor selection was supplied. Use repository tools to find the relevant code before answering.".to_owned(),
            |selection| format!(
                "Repository-relative file: {}\nLanguage: {}\nSelection: {}:{}-{}:{}\nSelected source (untrusted evidence):\n<selection>\n{}\n</selection>\n\nLSP-resolved symbol evidence already inspected by Helix (untrusted):\n{}\n\nAnswer directly without tools when this supplied evidence contains every fact the developer requested. If a fact is absent, use only the narrowest tool needed for that missing fact.",
                selection.relative_path.display(),
                selection.language.as_deref().unwrap_or("unknown"),
                selection.start_line,
                selection.start_column,
                selection.end_line,
                selection.end_column,
                selection_evidence.as_ref().map_or(selection.text.as_str(), |evidence| evidence.excerpt.as_str()),
                symbol_prompt,
            ),
        );
        let prompt = format!("{editor_context}\n\nDeveloper question: {query}");
        let evidence_fast_path = symbol_context.is_some();
        if evidence_fast_path {
            driver.set_thinking_level("off")?;
        }
        if let Err(cause) = driver.send(
            &serde_json::json!({"id":format!("lantern-turn-{id}"),"type":"prompt","message":prompt}),
        ) {
            if evidence_fast_path {
                let _ = driver.set_thinking_level("medium");
            }
            return Err(cause);
        }
        cancellation.attach_abort(driver.stdin.clone());

        let stream_result = (|| -> Result<bool, String> {
            let mut active_tools = HashMap::new();
            let mut escalated_reasoning = false;
            let mut stdout = driver.stdout.lock().expect("Pi stdout lock");
            loop {
                let mut line = String::new();
                let bytes = stdout
                    .read_line(&mut line)
                    .map_err(|cause| format!("cannot read Pi event: {cause}"))?;
                if bytes == 0 {
                    return Ok(false);
                }
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
                        return Err("Pi rejected the request; provider detail was excluded".into());
                    }
                    Some("tool_execution_start") => {
                        if evidence_fast_path && !escalated_reasoning {
                            driver.set_thinking_level("medium")?;
                            escalated_reasoning = true;
                        }
                        let call_id = event["toolCallId"]
                            .as_str()
                            .filter(|value| value.len() <= 256)
                            .ok_or("Pi tool call has an invalid identifier")?;
                        let name = event["toolName"]
                            .as_str()
                            .ok_or("Pi tool call has no tool name")?;
                        let tool = pi_tool(name)
                            .ok_or_else(|| format!("Pi requested unsupported tool `{name}`"))?;
                        let relative_path = pi_tool_path(&event, &root);
                        emit(
                            &writer,
                            &Event::ToolStarted {
                                id,
                                tool,
                                relative_path: relative_path.clone(),
                            },
                        )
                        .map_err(|cause| format!("cannot stream tool start: {cause}"))?;
                        active_tools.insert(call_id.to_owned(), (tool, relative_path));
                    }
                    Some("tool_execution_end") => {
                        let call_id = event["toolCallId"]
                            .as_str()
                            .ok_or("Pi tool result has no identifier")?;
                        let (tool, relative_path) = active_tools
                            .remove(call_id)
                            .ok_or("Pi completed a tool that was not started")?;
                        let success = event["isError"]
                            .as_bool()
                            .map(|is_error| !is_error)
                            .ok_or("Pi tool result has no error status")?;
                        emit(
                            &writer,
                            &Event::ToolFinished {
                                id,
                                tool,
                                relative_path,
                                success,
                            },
                        )
                        .map_err(|cause| format!("cannot stream tool result: {cause}"))?;
                    }
                    _ => {}
                }
            }
        })();
        let restore_result = if evidence_fast_path {
            driver.set_thinking_level("medium")
        } else {
            Ok(())
        };
        let saw_agent_settled = stream_result?;
        restore_result?;
        if !saw_agent_settled && !cancellation.is_requested() {
            return Err(
                "Pi closed before the agent turn settled; provider stderr was excluded".into(),
            );
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
        driver.stop();
        let _ = diagnose(
            &Record::new(
                Level::Error,
                Component::Provider,
                DiagnosticCode::ProviderFailed,
            )
            .for_operation(id),
        );
        error(
            &writer,
            Some(id),
            message,
            "run Pi interactively to inspect provider status, use `/login` for OpenAI Codex if required, then retry `/agent <question>`",
        );
    } else {
        let _ = emit(
            &writer,
            &Event::Completed {
                id,
                evidence_count: 1 + symbol_context.as_ref().map_or(0, |context| {
                    1 + context.references.len() + context.calls.len()
                }),
            },
        );
    }
    settle(id, &operations, &writer);
}

fn drain_provider_stderr(mut reader: impl Read) -> bool {
    let mut saw_output = false;
    let mut chunk = [0_u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(_) => saw_output = true,
            Err(_) => break,
        }
    }
    saw_output
}

fn pi_tool(name: &str) -> Option<WorkbenchTool> {
    match name {
        "read" => Some(WorkbenchTool::Read),
        "grep" => Some(WorkbenchTool::Grep),
        "find" => Some(WorkbenchTool::Find),
        "ls" => Some(WorkbenchTool::List),
        "edit" => Some(WorkbenchTool::Edit),
        "write" => Some(WorkbenchTool::Write),
        "bash" => Some(WorkbenchTool::Bash),
        _ => None,
    }
}

fn pi_tool_path(event: &serde_json::Value, root: &Path) -> Option<PathBuf> {
    let path = PathBuf::from(event.get("args")?.get("path")?.as_str()?);
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?.to_owned()
    } else {
        path
    };
    (relative.as_os_str().len() <= 4096 && validate_relative_path(&relative).is_ok())
        .then_some(relative)
}

fn persistent_pi(
    current: &mut Option<Arc<PiDriver>>,
    repository: &Path,
    writer: &SharedWriter,
    id: u64,
) -> Option<Arc<PiDriver>> {
    if let Some(driver) = current {
        if driver.root == repository {
            return Some(driver.clone());
        }
        error(
            writer,
            Some(id),
            "Pi driver belongs to a different workbench",
            "close and reopen Lantern in the intended repository",
        );
        return None;
    }
    match PiDriver::spawn(repository.to_owned()) {
        Ok(driver) => {
            let driver = Arc::new(driver);
            *current = Some(driver.clone());
            Some(driver)
        }
        Err(message) => {
            error(
                writer,
                Some(id),
                message,
                "run Pi interactively to inspect provider status, use `/login` for OpenAI Codex if required, then restart Lantern",
            );
            None
        }
    }
}

fn main() -> io::Result<()> {
    let _ = diagnose(&Record::new(
        Level::Info,
        Component::Daemon,
        DiagnosticCode::DaemonStarted,
    ));
    let writer = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    let operations: Operations = Arc::new(Mutex::new(HashMap::new()));
    let mut workers = Vec::new();
    let mut input = BufReader::new(io::stdin());
    let mut initialized = false;
    let mut workbench = None;
    let mut pi_driver: Option<Arc<PiDriver>> = None;

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
                        "rebuild the Lantern client and daemon from the same checkout",
                    );
                } else {
                    initialized = true;
                    let _ = diagnose(&Record::new(
                        Level::Info,
                        Component::Protocol,
                        DiagnosticCode::ProtocolInitialized,
                    ));
                    emit(
                        &writer,
                        &Event::Initialized {
                            protocol_version: PROTOCOL_VERSION,
                        },
                    )?;
                }
            }
            Request::OpenWorkbench { repository } => {
                if !operations.lock().expect("operations lock").is_empty() {
                    workspace_error(
                        &writer,
                        "workbench cannot change during an active operation",
                        "cancel or wait for the active operation, then open the workbench again",
                    );
                    continue;
                }
                let root = match canonical_repository(&repository) {
                    Ok(root) => root,
                    Err(message) => {
                        workspace_error(&writer, message, "open an existing repository and retry");
                        continue;
                    }
                };
                if workbench.as_ref().is_some_and(|current| current != &root)
                    && let Some(driver) = pi_driver.take()
                {
                    driver.stop();
                }
                workbench = Some(root.clone());
                let _ = diagnose(&Record::new(
                    Level::Info,
                    Component::Workbench,
                    DiagnosticCode::WorkbenchOpened,
                ));
                emit(&writer, &Event::WorkbenchOpened { repository: root })?;
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
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
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
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
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
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let Some(driver) = persistent_pi(&mut pi_driver, &repository, &writer, id) else {
                    settle(id, &operations, &writer);
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_pi_operation(
                        id,
                        driver,
                        query,
                        AgentContext::Selection(selection),
                        cancellation,
                        active_operations,
                        operation_writer,
                    );
                }));
            }
            Request::AskAgent {
                id,
                repository,
                query,
            } => {
                if query.trim().is_empty() {
                    error(
                        &writer,
                        Some(id),
                        "query is empty",
                        "type a question and retry",
                    );
                    continue;
                }
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let Some(driver) = persistent_pi(&mut pi_driver, &repository, &writer, id) else {
                    settle(id, &operations, &writer);
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_pi_operation(
                        id,
                        driver,
                        query,
                        AgentContext::Repository,
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
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
                let Some(cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let Some(driver) = persistent_pi(&mut pi_driver, &repository, &writer, id) else {
                    settle(id, &operations, &writer);
                    continue;
                };
                let operation_writer = writer.clone();
                let active_operations = operations.clone();
                workers.push(thread::spawn(move || {
                    run_pi_operation(
                        id,
                        driver,
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
                let Some(repository) = opened_repository(&workbench, &repository, &writer, id)
                else {
                    continue;
                };
                let Some(_cancellation) = admit(id, &operations, &writer) else {
                    continue;
                };
                let result = (|| -> Result<(), String> {
                    let root = canonical_repository(&repository)?;
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
    if let Some(driver) = pi_driver {
        driver.stop();
    }
    let _ = diagnose(&Record::new(
        Level::Info,
        Component::Daemon,
        DiagnosticCode::DaemonStopping,
    ));

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
    fn provider_stderr_is_drained_without_retaining_its_contents() {
        let diagnostics = vec![b'x'; 32 * 1024];
        assert!(drain_provider_stderr(diagnostics.as_slice()));
        assert!(!drain_provider_stderr([].as_slice()));
    }

    #[test]
    fn repository_search_skips_dependency_and_tool_cache_directories() {
        for directory in [
            ".git",
            ".lantern",
            ".pytest_cache",
            ".ruff_cache",
            ".venv",
            "__pycache__",
            "node_modules",
            "target",
            "venv",
        ] {
            assert!(
                should_skip(Path::new(directory)),
                "did not skip {directory}"
            );
        }
        assert!(!should_skip(Path::new("src")));
        assert!(!should_skip(Path::new("tests")));
    }
}
