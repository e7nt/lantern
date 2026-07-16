use lantern_protocol::{
    Capability, Event, Evidence, MAX_FRAME_BYTES, PROTOCOL_VERSION, Request, SelectionContext,
    SymbolContext, SymbolLocation,
};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Daemon {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Daemon {
    fn spawn() -> Self {
        Self::spawn_command(&mut Command::new(env!("CARGO_BIN_EXE_lantern-daemon")))
    }

    fn spawn_with_pi(pi_bin: &PathBuf, model_workdir: &PathBuf, mode: &str) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lantern-daemon"));
        command
            .env("LANTERN_PI_BIN", pi_bin)
            .env("LANTERN_PI_MODEL", "test-model")
            .env("LANTERN_MODEL_WORKDIR", model_workdir)
            .env("LANTERN_FAKE_PI_MODE", mode);
        Self::spawn_command(&mut command)
    }

    fn spawn_command(command: &mut Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn daemon");
        let stdin = BufWriter::new(child.stdin.take().expect("daemon stdin"));
        let stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, request: &Request) {
        serde_json::to_writer(&mut self.stdin, request).expect("serialize request");
        self.stdin.write_all(b"\n").expect("frame request");
        self.stdin.flush().expect("flush request");
    }

    fn send_raw(&mut self, bytes: &[u8]) {
        self.stdin.write_all(bytes).expect("write raw request");
        self.stdin.write_all(b"\n").expect("frame raw request");
        self.stdin.flush().expect("flush raw request");
    }

    fn next(&mut self) -> Event {
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read event");
        assert!(!line.is_empty(), "daemon closed before the expected event");
        serde_json::from_str(&line).expect("deserialize event")
    }

    fn initialize(&mut self) {
        self.send(&Request::Initialize {
            protocol_version: PROTOCOL_VERSION,
        });
        assert!(matches!(self.next(), Event::Initialized { .. }));
    }

    fn configure(&mut self, repository: &Path, capabilities: Vec<Capability>) {
        self.send(&Request::ConfigureWorkspace {
            repository: repository.to_owned(),
            capabilities: capabilities.clone(),
        });
        match self.next() {
            Event::WorkspaceConfigured {
                repository: configured,
                capabilities: configured_capabilities,
            } => {
                assert_eq!(
                    configured,
                    repository.canonicalize().expect("canonical root")
                );
                assert_eq!(configured_capabilities, capabilities);
            }
            event => panic!("expected workspace configuration, received {event:?}"),
        }
    }

    fn trust_read(&mut self, repository: &Path) {
        self.configure(repository, vec![Capability::RepositoryRead]);
    }

    fn trust_model(&mut self, repository: &Path) {
        self.configure(
            repository,
            vec![Capability::RepositoryRead, Capability::NetworkAccess],
        );
    }
}

#[test]
fn golden_wire_fixtures_match_the_v3_types() {
    for line in include_str!("../../../protocol/v3/requests.jsonl").lines() {
        serde_json::from_str::<Request>(line).expect("golden request must deserialize");
    }
    for line in include_str!("../../../protocol/v3/events.jsonl").lines() {
        serde_json::from_str::<Event>(line).expect("golden event must deserialize");
    }
}

#[test]
fn admits_before_execution_and_settles_after_the_outcome() {
    let root = fixture("lifecycle", "lifecycle evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 20,
        repository: root.clone(),
        query: "lifecycle".into(),
    });

    assert!(matches!(daemon.next(), Event::Accepted { id: 20 }));
    assert!(matches!(
        daemon.next(),
        Event::OperationStarted { id: 20, .. }
    ));
    let outcome = loop {
        if let event @ (Event::Completed { id: 20, .. }
        | Event::Cancelled { id: 20, .. }
        | Event::Error { id: Some(20), .. }) = daemon.next()
        {
            break event;
        }
    };
    assert!(matches!(outcome, Event::Completed { .. }));
    assert!(matches!(daemon.next(), Event::Settled { id: 20 }));
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn a_duplicate_active_id_is_rejected_without_replacing_the_operation() {
    let root = fixture("duplicate", "duplicate evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    let request = Request::Ask {
        id: 21,
        repository: root.clone(),
        query: "duplicate".into(),
    };
    daemon.send(&request);
    daemon.send(&request);

    let mut accepted = 0;
    let mut started = 0;
    let mut rejected = false;
    let mut settled = false;
    while !rejected || !settled {
        match daemon.next() {
            Event::Accepted { id: 21 } => accepted += 1,
            Event::OperationStarted { id: 21, .. } => started += 1,
            Event::Error {
                id: Some(21),
                message,
                ..
            } if message.contains("already active") => rejected = true,
            Event::Settled { id: 21 } => settled = true,
            _ => {}
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(started, 1);
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn a_second_operation_is_rejected_until_the_first_settles() {
    let root = fixture("single-operation", "first evidence\nsecond evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 25,
        repository: root.clone(),
        query: "first".into(),
    });
    assert!(matches!(daemon.next(), Event::Accepted { id: 25 }));
    daemon.send(&Request::Ask {
        id: 26,
        repository: root.clone(),
        query: "second".into(),
    });

    let mut rejected = false;
    let mut settled = false;
    while !rejected || !settled {
        match daemon.next() {
            Event::Error {
                id: Some(26),
                message,
                ..
            } if message.contains("another operation") => rejected = true,
            Event::Settled { id: 25 } => settled = true,
            Event::Accepted { id: 26 } => panic!("second operation was admitted"),
            _ => {}
        }
    }
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn malformed_json_does_not_poison_the_next_frame() {
    let mut daemon = Daemon::spawn();
    daemon.send_raw(b"{not-json}");
    assert!(matches!(daemon.next(), Event::Error { id: None, .. }));
    daemon.initialize();
}

#[test]
fn cancelling_an_idle_operation_is_an_idempotent_no_op() {
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.send(&Request::Cancel { id: 404 });
    daemon.initialize();
}

#[test]
fn rejects_unknown_fields_and_recovers_at_the_next_frame() {
    let mut daemon = Daemon::spawn();
    daemon.send_raw(br#"{"method":"initialize","protocol_version":3,"surprise":true}"#);
    assert!(matches!(daemon.next(), Event::Error { id: None, .. }));
    daemon.initialize();
}

#[test]
fn rejects_work_before_a_successful_version_handshake() {
    let root = fixture("pre-initialization", "not admitted\n");
    let mut daemon = Daemon::spawn();
    daemon.send(&Request::Ask {
        id: 24,
        repository: root.clone(),
        query: "not admitted".into(),
    });
    match daemon.next() {
        Event::Error {
            id: None, message, ..
        } => {
            assert!(message.contains("not initialized"));
        }
        event => panic!("expected initialization error, received {event:?}"),
    }
    daemon.initialize();
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn an_oversized_frame_is_drained_before_the_next_request() {
    let mut daemon = Daemon::spawn();
    daemon.send_raw(&vec![b'x'; MAX_FRAME_BYTES + 1]);
    match daemon.next() {
        Event::Error { message, .. } => assert!(message.contains("exceeds")),
        event => panic!("expected frame error, received {event:?}"),
    }
    daemon.initialize();
}

#[test]
fn workspace_starts_locked_and_denies_before_operation_admission() {
    let root = fixture("locked", "private evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.configure(&root, vec![]);
    daemon.send(&Request::Ask {
        id: 29,
        repository: root.clone(),
        query: "private".into(),
    });

    match daemon.next() {
        Event::Error {
            id: Some(29),
            message,
            recovery,
        } => {
            assert!(message.contains("repository read access is not granted"));
            assert!(recovery.contains("/trust read"));
        }
        event => panic!("expected a read denial, received {event:?}"),
    }
    daemon.trust_read(&root);
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn model_transmission_requires_its_separate_capability() {
    let root = fixture("transmission-denied", "fn selected() {}\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 30,
        repository: root.clone(),
        query: "Explain this".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });

    match daemon.next() {
        Event::Error {
            id: Some(30),
            message,
            recovery,
        } => {
            assert!(message.contains("model transmission is not granted"));
            assert!(recovery.contains("/trust model"));
        }
        event => panic!("expected a transmission denial, received {event:?}"),
    }
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn quick_ask_rejects_write_and_execution_grants() {
    let root = fixture("unsupported-grants", "unchanged\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    for capability in [Capability::RepositoryWrite, Capability::ProcessExecution] {
        daemon.send(&Request::ConfigureWorkspace {
            repository: root.clone(),
            capabilities: vec![capability],
        });
        match daemon.next() {
            Event::WorkspaceConfigurationFailed { message, .. } => {
                assert!(message.contains("unavailable in read-only Quick Ask"));
            }
            event => panic!("expected a hard capability denial, received {event:?}"),
        }
    }
    assert_eq!(
        fs::read_to_string(root.join("sample.rs")).expect("read fixture"),
        "unchanged\n"
    );
    daemon.configure(&root, vec![]);
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn configured_workspace_cannot_be_retargeted() {
    let root = fixture("bound-root", "root\n");
    let other = fixture("other-root", "other\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.configure(&root, vec![]);
    daemon.send(&Request::ConfigureWorkspace {
        repository: other.clone(),
        capabilities: vec![],
    });
    match daemon.next() {
        Event::WorkspaceConfigurationFailed { message, .. } => {
            assert!(message.contains("workspace is bound"));
        }
        event => panic!("expected repository retarget denial, received {event:?}"),
    }
    fs::remove_dir_all(root).expect("remove root fixture");
    fs::remove_dir_all(other).expect("remove other fixture");
}

#[test]
fn trust_cannot_change_until_the_active_operation_settles() {
    let root = fixture("trust-during-operation", "active evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 31,
        repository: root.clone(),
        query: "active".into(),
    });
    assert!(matches!(daemon.next(), Event::Accepted { id: 31 }));
    daemon.send(&Request::ConfigureWorkspace {
        repository: root.clone(),
        capabilities: vec![],
    });

    let mut rejected_change = false;
    let mut settled = false;
    while !rejected_change || !settled {
        match daemon.next() {
            Event::WorkspaceConfigurationFailed { message, .. } => {
                assert!(message.contains("active operation"));
                rejected_change = true;
            }
            Event::Settled { id: 31 } => settled = true,
            _ => {}
        }
    }

    daemon.send(&Request::Ask {
        id: 32,
        repository: root.clone(),
        query: "active".into(),
    });
    assert!(matches!(daemon.next(), Event::Accepted { id: 32 }));
    daemon.send(&Request::Cancel { id: 32 });
    while !matches!(daemon.next(), Event::Settled { id: 32 }) {}
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn unicode_line_separators_remain_inside_one_jsonl_frame() {
    let root = fixture("unicode-separator", "before\nafter\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 22,
        repository: root.clone(),
        query: "before\u{2028}after".into(),
    });
    assert!(matches!(daemon.next(), Event::Accepted { id: 22 }));
    while !matches!(daemon.next(), Event::Settled { id: 22 }) {}
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn shutdown_cancels_and_settles_workers_before_process_exit() {
    let root = fixture("shutdown", "shutdown evidence\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_lantern-daemon"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let mut stdin = BufWriter::new(child.stdin.take().expect("daemon stdin"));
    let mut stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));

    for request in [
        Request::Initialize {
            protocol_version: PROTOCOL_VERSION,
        },
        Request::ConfigureWorkspace {
            repository: root.clone(),
            capabilities: vec![Capability::RepositoryRead],
        },
        Request::Ask {
            id: 23,
            repository: root.clone(),
            query: "shutdown".into(),
        },
    ] {
        serde_json::to_writer(&mut stdin, &request).expect("serialize request");
        stdin.write_all(b"\n").expect("frame request");
    }
    stdin.flush().expect("flush requests");

    let mut line = String::new();
    stdout.read_line(&mut line).expect("read initialization");
    line.clear();
    stdout
        .read_line(&mut line)
        .expect("read workspace configuration");
    assert!(matches!(
        serde_json::from_str::<Event>(&line).expect("decode workspace configuration"),
        Event::WorkspaceConfigured { .. }
    ));
    line.clear();
    stdout.read_line(&mut line).expect("read acceptance");
    assert!(matches!(
        serde_json::from_str::<Event>(&line).expect("decode acceptance"),
        Event::Accepted { id: 23 }
    ));

    serde_json::to_writer(&mut stdin, &Request::Shutdown).expect("serialize shutdown");
    stdin.write_all(b"\n").expect("frame shutdown");
    stdin.flush().expect("flush shutdown");
    drop(stdin);
    let status = child.wait().expect("wait for daemon");
    assert!(status.success());

    let mut remaining = String::new();
    stdout
        .read_to_string(&mut remaining)
        .expect("read shutdown events");
    let events: Vec<Event> = remaining
        .lines()
        .map(|line| serde_json::from_str(line).expect("decode shutdown event"))
        .collect();
    let cancelled = events
        .iter()
        .position(|event| matches!(event, Event::Cancelled { id: 23, .. }))
        .expect("shutdown cancellation");
    let settled = events
        .iter()
        .position(|event| matches!(event, Event::Settled { id: 23 }))
        .expect("shutdown settlement");
    assert!(cancelled < settled);
    fs::remove_dir_all(root).expect("remove fixture");
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.send(&Request::Shutdown);
        let _ = self.child.wait();
    }
}

fn fixture(name: &str, contents: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "lantern-daemon-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("create fixture");
    fs::write(root.join("sample.rs"), contents).expect("write fixture");
    root
}

#[cfg(unix)]
fn fake_pi(root: &Path) -> PathBuf {
    let path = root.join("fake-pi");
    fs::write(
        &path,
        r#"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" > invocation.args
printf '%s\n' "$$" > pi.pid
IFS= read -r prompt
printf '%s\n' "$prompt" > prompt.json
printf '%s\n' '{"type":"response","command":"prompt","success":true}'
if [[ ${LANTERN_FAKE_PI_MODE:?} == malformed ]]; then
    printf '%s\n' 'not-json'
    while :; do
        read -r -t 1 _ || true
    done
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == stderr-flood ]]; then
    head -c 131072 /dev/zero | tr '\0' x >&2
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == cancel ]]; then
    IFS= read -r abort
    printf '%s\n' "$abort" > abort.json
else
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Evidence-grounded "}}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"answer."}}'
fi
printf '%s\n' '{"type":"agent_end","willRetry":false}'
printf '%s\n' '{"type":"agent_settled"}'
"#,
    )
    .expect("write fake Pi");
    let mut permissions = fs::metadata(&path).expect("fake Pi metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake Pi executable");
    path
}

#[test]
fn streams_an_exact_evidence_range() {
    let root = fixture("evidence", "alpha\nneedle here\nomega\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 1,
        repository: root.clone(),
        query: "needle".into(),
    });

    let evidence = loop {
        if let Event::Evidence { evidence, .. } = daemon.next() {
            break evidence;
        }
    };
    assert_eq!(
        evidence,
        Evidence {
            relative_path: "sample.rs".into(),
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 7,
            excerpt: "needle here".into(),
        }
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn acknowledges_cancellation_within_budget() {
    let root = fixture("cancellation", "interruptible evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 7,
        repository: root.clone(),
        query: "interruptible".into(),
    });

    loop {
        if matches!(daemon.next(), Event::OperationStarted { id: 7, .. }) {
            break;
        }
    }
    daemon.send(&Request::Cancel { id: 7 });

    let latency = loop {
        if let Event::Cancelled {
            id: 7,
            cancellation_latency_ms,
        } = daemon.next()
        {
            break cancellation_latency_ms;
        }
    };
    assert!(
        Duration::from_millis(latency) < Duration::from_millis(500),
        "cancellation took {latency} ms"
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn accepts_selection_context_as_exact_evidence() {
    let root = fixture("selection", "fn selected() {}\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::AskSelection {
        id: 9,
        repository: root.clone(),
        query: "What does this do?".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });

    let evidence = loop {
        if let Event::Evidence { evidence, .. } = daemon.next() {
            break evidence;
        }
    };
    assert_eq!(evidence.relative_path, PathBuf::from("sample.rs"));
    assert_eq!((evidence.start_column, evidence.end_column), (1, 17));
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn previews_a_replacement_without_modifying_the_file() {
    let root = fixture("preview", "old text\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::PreviewSelection {
        id: 10,
        repository: root.clone(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 9,
            text: "old text".into(),
            document_modified: false,
        },
        replacement: "new text".into(),
    });
    let proposal = loop {
        if let Event::ChangeProposal { proposal, .. } = daemon.next() {
            break proposal;
        }
    };
    assert_eq!(proposal.replacement, "new text");
    assert_eq!(
        fs::read_to_string(root.join("sample.rs")).unwrap(),
        "old text\n"
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[cfg(unix)]
#[test]
fn streams_pi_rpc_without_putting_source_in_process_arguments() {
    let root = fixture("pi-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stream");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 11,
        repository: root.clone(),
        query: "Explain the handoff".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });

    let mut answer = String::new();
    loop {
        match daemon.next() {
            Event::TextDelta { id: 11, delta } => answer.push_str(&delta),
            Event::Completed { id: 11, .. } => break,
            _ => {}
        }
    }
    assert_eq!(answer, "Evidence-grounded answer.");
    let arguments = fs::read_to_string(model_workdir.join("invocation.args")).unwrap();
    assert!(!arguments.contains("fn selected"));
    assert!(!arguments.contains("Explain the handoff"));
    let prompt = fs::read_to_string(model_workdir.join("prompt.json")).unwrap();
    assert!(prompt.contains("fn selected() {}"));
    assert_eq!(
        fs::read_to_string(root.join("sample.rs")).unwrap(),
        "fn selected() {}\n"
    );
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn continuously_drains_and_bounds_pi_stderr() {
    let root = fixture("pi-stderr-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-stderr-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stderr-flood");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 27,
        repository: root.clone(),
        query: "Explain this".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });

    while !matches!(daemon.next(), Event::Settled { id: 27 }) {}
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn reaps_pi_after_a_malformed_stream_event() {
    let root = fixture("pi-malformed-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-malformed-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "malformed");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 28,
        repository: root.clone(),
        query: "Explain this".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });

    let mut reported = false;
    loop {
        match daemon.next() {
            Event::Error {
                id: Some(28),
                message,
                ..
            } if message.contains("invalid JSON") => reported = true,
            Event::Settled { id: 28 } => break,
            _ => {}
        }
    }
    assert!(reported, "malformed Pi event was not reported");
    let pid = fs::read_to_string(model_workdir.join("pi.pid")).expect("read Pi pid");
    let alive = Command::new("kill")
        .args(["-0", pid.trim()])
        .output()
        .expect("probe Pi process")
        .status
        .success();
    if alive {
        let _ = Command::new("kill").args(["-9", pid.trim()]).status();
    }
    assert!(!alive, "Pi process {pid} survived malformed stream cleanup");
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn streams_definition_and_references_before_a_symbol_grounded_answer() {
    let root = fixture("symbol-repository", "fn caller() { resolved(); }\n");
    fs::write(root.join("definition.rs"), "fn resolved() {}\n").expect("write definition");
    let model_workdir = fixture("symbol-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stream");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSymbol {
        id: 13,
        repository: root.clone(),
        query: "Where is this defined and used?".into(),
        context: SymbolContext {
            selection: SelectionContext {
                relative_path: "sample.rs".into(),
                language: Some("rust".into()),
                start_line: 1,
                start_column: 15,
                end_line: 1,
                end_column: 23,
                text: "resolved".into(),
                document_modified: false,
            },
            definition: SymbolLocation {
                relative_path: "definition.rs".into(),
                start_line: 1,
                start_column: 4,
                end_line: 1,
                end_column: 12,
            },
            references: vec![SymbolLocation {
                relative_path: "sample.rs".into(),
                start_line: 1,
                start_column: 15,
                end_line: 1,
                end_column: 23,
            }],
        },
    });

    let mut locations = Vec::new();
    loop {
        match daemon.next() {
            Event::Evidence { id: 13, evidence } => locations.push(evidence.relative_path),
            Event::Completed {
                id: 13,
                evidence_count,
            } => {
                assert_eq!(evidence_count, 3);
                break;
            }
            _ => {}
        }
    }
    assert_eq!(
        locations,
        vec![
            PathBuf::from("sample.rs"),
            PathBuf::from("definition.rs"),
            PathBuf::from("sample.rs"),
        ]
    );
    let prompt = fs::read_to_string(model_workdir.join("prompt.json")).unwrap();
    assert!(prompt.contains("<definition path=\\\"definition.rs\\\""));
    assert!(prompt.contains("<reference path=\\\"sample.rs\\\""));
    assert!(prompt.contains("fn resolved() {}"));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn aborts_an_active_pi_rpc_turn_within_budget() {
    let root = fixture("pi-cancel-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-cancel-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "cancel");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 12,
        repository: root.clone(),
        query: "Explain this".into(),
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 17,
            text: "fn selected() {}".into(),
            document_modified: false,
        },
    });
    loop {
        if matches!(daemon.next(), Event::OperationStarted { id: 12, .. }) {
            break;
        }
    }
    daemon.send(&Request::Cancel { id: 12 });
    let latency = loop {
        if let Event::Cancelled {
            id: 12,
            cancellation_latency_ms,
        } = daemon.next()
        {
            break cancellation_latency_ms;
        }
    };
    assert!(latency < 500, "Pi cancellation took {latency} ms");
    let abort = fs::read_to_string(model_workdir.join("abort.json")).unwrap();
    assert!(abort.contains(r#""type":"abort""#));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}
