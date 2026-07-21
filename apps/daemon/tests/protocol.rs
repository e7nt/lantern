use lantern_diagnostics::{Code as DiagnosticCode, Record as DiagnosticRecord};
use lantern_protocol::{
    AgentIntent, CodeReviewAnchor, CodeReviewComment, Event, Evidence, EvidenceSource,
    GitReviewContext, GitReviewScope, GitReviewState, GroundingState, MAX_FILES, MAX_FRAME_BYTES,
    PROTOCOL_VERSION, PlanReviewComment, Request, SelectionContext, SymbolCall, SymbolContext,
    SymbolLocation, WorkbenchTool,
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
            .stderr(Stdio::null())
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

    fn open(&mut self, repository: &Path) {
        self.send(&Request::OpenWorkbench {
            repository: repository.to_owned(),
        });
        match self.next() {
            Event::WorkbenchOpened {
                repository: configured,
            } => {
                assert_eq!(
                    configured,
                    repository.canonicalize().expect("canonical root")
                );
            }
            event => panic!("expected workbench to open, received {event:?}"),
        }
    }

    fn trust_read(&mut self, repository: &Path) {
        self.open(repository);
    }

    fn trust_model(&mut self, repository: &Path) {
        self.open(repository);
    }
}

#[test]
fn golden_wire_fixtures_match_the_v16_types() {
    for line in include_str!("../../../protocol/v16/requests.jsonl").lines() {
        serde_json::from_str::<Request>(line).expect("golden request must deserialize");
    }
    for line in include_str!("../../../protocol/v16/events.jsonl").lines() {
        serde_json::from_str::<Event>(line).expect("golden event must deserialize");
    }
}

#[test]
fn diagnostics_are_structured_metadata_without_repository_content() {
    let root = fixture("diagnostic-redaction", "sk-sensitive-source\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_lantern-daemon"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let mut stdin = BufWriter::new(child.stdin.take().expect("daemon stdin"));
    let mut stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));
    let mut stderr = child.stderr.take().expect("daemon stderr");
    let stderr_reader = std::thread::spawn(move || {
        let mut diagnostic_jsonl = String::new();
        stderr
            .read_to_string(&mut diagnostic_jsonl)
            .expect("read diagnostics");
        diagnostic_jsonl
    });
    for request in [
        Request::Initialize {
            protocol_version: PROTOCOL_VERSION,
        },
        Request::OpenWorkbench {
            repository: root.clone(),
        },
        Request::AskSelection {
            id: 33,
            repository: root.clone(),
            query: "question-with-private-marker".into(),
            selection: SelectionContext {
                relative_path: "sample.rs".into(),
                language: Some("rust".into()),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 20,
                text: "sk-sensitive-source".into(),
                document_modified: false,
            },
        },
    ] {
        serde_json::to_writer(&mut stdin, &request).expect("serialize request");
        stdin.write_all(b"\n").expect("frame request");
    }
    stdin.flush().expect("flush requests");

    let mut line = String::new();
    loop {
        line.clear();
        stdout.read_line(&mut line).expect("read daemon event");
        let event: Event = serde_json::from_str(&line).expect("decode daemon event");
        if matches!(event, Event::Settled { id: 33 }) {
            break;
        }
    }
    serde_json::to_writer(&mut stdin, &Request::Shutdown).expect("serialize shutdown");
    stdin.write_all(b"\n").expect("frame shutdown");
    stdin.flush().expect("flush shutdown");
    drop(stdin);
    assert!(child.wait().expect("wait for daemon").success());

    let diagnostic_jsonl = stderr_reader.join().expect("join diagnostic reader");
    assert!(!diagnostic_jsonl.contains("sk-sensitive-source"));
    assert!(!diagnostic_jsonl.contains("question-with-private-marker"));
    assert!(!diagnostic_jsonl.contains(&root.to_string_lossy().to_string()));
    let records = diagnostic_jsonl
        .lines()
        .map(|line| serde_json::from_str::<DiagnosticRecord>(line).expect("structured record"))
        .collect::<Vec<_>>();
    assert!(
        records
            .iter()
            .any(|record| record.code == DiagnosticCode::OperationAccepted)
    );
    assert!(
        records
            .iter()
            .any(|record| record.code == DiagnosticCode::OperationSettled)
    );
    fs::remove_dir_all(root).expect("remove fixture");
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
    daemon.send_raw(br#"{"method":"initialize","protocol_version":4,"surprise":true}"#);
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
fn work_requires_an_open_workbench() {
    let root = fixture("unopened", "private evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
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
            assert!(message.contains("does not belong to the open workbench"));
            assert!(recovery.contains("open the repository"));
        }
        event => panic!("expected an unopened-workbench denial, received {event:?}"),
    }
    daemon.open(&root);
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn requests_cannot_escape_the_open_workbench() {
    let root = fixture("bound-root", "root\n");
    let other = fixture("other-root", "other\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::Ask {
        id: 30,
        repository: other.clone(),
        query: "other".into(),
    });
    match daemon.next() {
        Event::Error {
            id: Some(30),
            message,
            ..
        } => {
            assert!(message.contains("does not belong to the open workbench"));
        }
        event => panic!("expected repository boundary denial, received {event:?}"),
    }
    fs::remove_dir_all(root).expect("remove root fixture");
    fs::remove_dir_all(other).expect("remove other fixture");
}

#[test]
fn workbench_cannot_change_until_the_active_operation_settles() {
    let root = fixture("workbench-during-operation", "active evidence\n");
    let mut daemon = Daemon::spawn();
    daemon.initialize();
    daemon.trust_read(&root);
    daemon.send(&Request::Ask {
        id: 31,
        repository: root.clone(),
        query: "active".into(),
    });
    assert!(matches!(daemon.next(), Event::Accepted { id: 31 }));
    daemon.send(&Request::OpenWorkbench {
        repository: root.clone(),
    });

    let mut rejected_change = false;
    let mut settled = false;
    while !rejected_change || !settled {
        match daemon.next() {
            Event::WorkbenchOpenFailed { message, .. } => {
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
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let mut stdin = BufWriter::new(child.stdin.take().expect("daemon stdin"));
    let mut stdout = BufReader::new(child.stdout.take().expect("daemon stdout"));

    for request in [
        Request::Initialize {
            protocol_version: PROTOCOL_VERSION,
        },
        Request::OpenWorkbench {
            repository: root.clone(),
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
        Event::WorkbenchOpened { .. }
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
    let outcomes: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            matches!(
                event,
                Event::Completed { id: 23, .. }
                    | Event::Cancelled { id: 23, .. }
                    | Event::Error { id: Some(23), .. }
            )
        })
        .collect();
    assert_eq!(
        outcomes.len(),
        1,
        "operation must have one terminal outcome"
    );
    let settled = events
        .iter()
        .position(|event| matches!(event, Event::Settled { id: 23 }))
        .expect("shutdown settlement");
    assert!(outcomes[0].0 < settled);
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
capture_dir=$(cd "$(dirname "$0")" && pwd)
printf '%s\n' "$*" > "$capture_dir/invocation.args"
printf '%s\n' "$$" > "$capture_dir/pi.pid"
if [[ ${LANTERN_FAKE_PI_MODE:?} == persistent-cancel ]]; then
    IFS= read -r prompt
    printf '%s\n' "$prompt" >> "$capture_dir/prompts.jsonl"
    printf '%s\n' '{"type":"response","command":"prompt","success":true}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-read","toolName":"read","args":{"path":"sample.rs"}}'
    IFS= read -r abort
    printf '%s\n' "$abort" > "$capture_dir/abort.json"
    printf '%s\n' '{"type":"response","command":"abort","success":true}'
    printf '%s\n' '{"type":"agent_settled"}'
    IFS= read -r prompt
    printf '%s\n' "$prompt" >> "$capture_dir/prompts.jsonl"
    printf '%s\n' '{"type":"response","command":"prompt","success":true}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"continued after cancellation"}}'
    printf '%s\n' '{"type":"agent_settled"}'
    while IFS= read -r _; do :; done
    exit 0
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == persistent ]]; then
    turn=0
    while IFS= read -r prompt; do
        turn=$((turn + 1))
        printf '%s\n' "$prompt" >> "$capture_dir/prompts.jsonl"
        printf '%s\n' '{"type":"response","command":"prompt","success":true}'
        printf '%s\n' "{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"warm turn $turn\"}}"
        printf '%s\n' '{"type":"agent_end","willRetry":false}'
        printf '%s\n' '{"type":"agent_settled"}'
    done
    exit 0
fi
IFS= read -r prompt
reasoning_fast_path=0
if [[ $prompt == *'"type":"set_thinking_level"'* ]]; then
    reasoning_fast_path=1
    printf '%s\n' "$prompt" >> "$capture_dir/thinking.jsonl"
    printf '%s\n' '{"type":"response","command":"set_thinking_level","success":true}'
    IFS= read -r prompt
fi
printf '%s\n' "$prompt" > "$capture_dir/prompt.json"
printf '%s\n' '{"type":"response","command":"prompt","success":true}'
if [[ ${LANTERN_FAKE_PI_MODE:?} == rejected ]]; then
    printf '%s\n' '{"type":"response","command":"prompt","success":false,"error":"credential sk-provider-response-secret was rejected"}'
    exit 0
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == stderr-close ]]; then
    printf '%s\n' 'sk-provider-secret' >&2
    exit 0
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == malformed ]]; then
    printf '%s\n' 'not-json'
    while :; do
        read -r -t 1 _ || true
    done
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == stderr-flood ]]; then
    head -c 131072 /dev/zero | tr '\0' x >&2
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == reasoning-escalation ]]; then
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-read","toolName":"read","args":{"path":"sample.rs"}}'
    IFS= read -r thinking
    printf '%s\n' "$thinking" >> "$capture_dir/thinking.jsonl"
    printf '%s\n' '{"type":"response","command":"set_thinking_level","success":true}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-read","toolName":"read","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"targeted answer"}}'
    printf '%s\n' '{"type":"agent_settled"}'
    IFS= read -r thinking
    printf '%s\n' "$thinking" >> "$capture_dir/thinking.jsonl"
    printf '%s\n' '{"type":"response","command":"set_thinking_level","success":true}'
    exit 0
fi
if [[ ${LANTERN_FAKE_PI_MODE:?} == cancel ]]; then
    IFS= read -r abort
    printf '%s\n' "$abort" > "$capture_dir/abort.json"
elif [[ ${LANTERN_FAKE_PI_MODE:?} == investigation ]]; then
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-read","toolName":"read","args":{"path":"sample.rs"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-read","toolName":"read","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Goal\nObserved\nRead existing flow.\nAffected flow\nrequest\nLikely changes\nnone yet\nOpen questions\nconfiguration\nAcceptance criteria\nexplicit behavior\nExclusions\nimplementation\nRisks\nstale configuration\nReadiness\nBlocked"}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == plan ]]; then
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Objective\nPersist the accepted plan.\nRepository evidence\n- apps/daemon/src/main.rs\nAcceptance criteria\n- Create one editable Markdown file.\nExclusions\n- Task dashboard.\nDecisions\n- Use create-new semantics.\nTasks\n1. Serialize the plan.\nRisks and unknowns\n- Existing active plan.\nVerification\n- Prove byte-identical duplicate rejection."}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == plan-review ]]; then
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Objective\nRevised measurable objective.\nRepository evidence\n- src/lib.rs:1\nAcceptance criteria\n- One measurable check.\nExclusions\n- Deferred task.\nDecisions\n- Preserve the first task.\nTasks\n- First task.\nRisks and unknowns\n- One risk.\nVerification\n- Run one test."}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == plan-progress && "$*" != *'read,grep,find,ls,edit'* ]]; then
    printf '%s\n' "$prompt" > "$capture_dir/progress-prompt.json"
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Objective\nUpdated value checkpoint.\nRepository evidence\n- sample.rs:1\nAcceptance criteria\n- Value returns two.\nExclusions\n- No unrelated changes.\nDecisions\n- Preserve the focused implementation.\nTasks\n- [x] Change the value.\nRisks and unknowns\n- None discovered.\nVerification\n- Focused verification completed."}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == plan-progress ]]; then
    printf '%s\n' "$prompt" > "$capture_dir/implementation-prompt.json"
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-edit","toolName":"edit","args":{"path":"sample.rs"}}'
    printf '%s\n' 'fn value() -> u8 { 2 }' > sample.rs
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-edit","toolName":"edit","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-test","toolName":"bash","args":{"command":"true"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-test","toolName":"bash","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Changed the value and verified it."}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == code-review ]]; then
    printf '%s\n' "$prompt" > "$capture_dir/code-review-prompt.json"
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-edit","toolName":"edit","args":{"path":"sample.rs"}}'
    printf '%s\n' 'fn value() -> u8 { 3 }' > sample.rs
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-edit","toolName":"edit","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-test","toolName":"bash","args":{"command":"true"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-test","toolName":"bash","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Addressed both review comments and verified the correction."}}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == tools ]]; then
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-read","toolName":"read","args":{"path":"sample.rs"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-read","toolName":"read","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-edit","toolName":"edit","args":{"path":"sample.rs"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-edit","toolName":"edit","result":{"content":[]},"isError":false}'
elif [[ ${LANTERN_FAKE_PI_MODE:?} == journey ]]; then
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-read","toolName":"read","args":{"path":"sample.rs"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-read","toolName":"read","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-edit","toolName":"edit","args":{"path":"sample.rs"}}'
    printf '%s\n' 'pub fn greeting() -> &str { "new" }' > sample.rs
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-edit","toolName":"edit","result":{"content":[]},"isError":false}'
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call-test","toolName":"bash","args":{"command":"./focused-test.sh"}}'
    if ./focused-test.sh; then
        printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-test","toolName":"bash","result":{"content":[]},"isError":false}'
    else
        printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call-test","toolName":"bash","result":{"content":[]},"isError":true}'
    fi
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Updated the greeting and verified it with the focused test."}}'
else
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Evidence-grounded "}}'
    printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"answer."}}'
fi
printf '%s\n' '{"type":"agent_end","willRetry":false}'
printf '%s\n' '{"type":"agent_settled"}'
if [[ $reasoning_fast_path == 1 ]]; then
    IFS= read -r thinking
    printf '%s\n' "$thinking" >> "$capture_dir/thinking.jsonl"
    printf '%s\n' '{"type":"response","command":"set_thinking_level","success":true}'
fi
"#,
    )
    .expect("write fake Pi");
    let mut permissions = fs::metadata(&path).expect("fake Pi metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake Pi executable");
    path
}

#[cfg(unix)]
fn await_thinking_levels(root: &Path, count: usize) -> Vec<String> {
    let path = root.join("thinking.jsonl");
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let commands = loop {
        let commands = fs::read_to_string(&path).expect("read thinking-level commands");
        if commands.lines().count() == count || std::time::Instant::now() >= deadline {
            break commands;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    commands
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("decode thinking command")
                ["level"]
                .as_str()
                .expect("thinking level")
                .to_owned()
        })
        .collect()
}

#[test]
fn reuses_one_pi_process_for_sequential_agent_turns() {
    let root = fixture("persistent-pi", "fn context() {}\n");
    let pi = fake_pi(&root);
    let mut daemon = Daemon::spawn_with_pi(&pi, &root, "persistent");
    daemon.initialize();
    daemon.trust_model(&root);

    let mut answers = Vec::new();
    for (id, query) in [
        (70, "inspect the context"),
        (71, "continue from that context"),
    ] {
        daemon.send(&Request::AskAgent {
            id,
            repository: root.clone(),
            query: query.into(),
            intent: AgentIntent::Implement,
        });
        let mut answer = String::new();
        loop {
            match daemon.next() {
                Event::TextDelta {
                    id: event_id,
                    delta,
                } if event_id == id => answer.push_str(&delta),
                Event::Settled { id: event_id } if event_id == id => break,
                _ => {}
            }
        }
        answers.push(answer);
    }

    assert_eq!(answers, ["warm turn 1", "warm turn 2"]);
    assert_eq!(
        fs::read_to_string(root.join("prompts.jsonl"))
            .expect("read captured prompts")
            .lines()
            .count(),
        2
    );
    let pid = fs::read_to_string(root.join("pi.pid")).expect("read Pi pid");
    daemon.send(&Request::Shutdown);
    drop(daemon);
    let status = Command::new("kill")
        .args(["-0", pid.trim()])
        .output()
        .expect("probe Pi process")
        .status;
    assert!(
        !status.success(),
        "Pi process {pid} survived daemon shutdown"
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn reuses_the_pi_process_after_cancelling_a_turn() {
    let root = fixture("persistent-pi-cancel", "fn context() {}\n");
    let pi = fake_pi(&root);
    let mut daemon = Daemon::spawn_with_pi(&pi, &root, "persistent-cancel");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgent {
        id: 72,
        repository: root.clone(),
        query: "start inspection".into(),
        intent: AgentIntent::Implement,
    });
    while !matches!(daemon.next(), Event::ToolStarted { id: 72, .. }) {}
    daemon.send(&Request::Cancel { id: 72 });
    while !matches!(daemon.next(), Event::Settled { id: 72 }) {}

    daemon.send(&Request::AskAgent {
        id: 73,
        repository: root.clone(),
        query: "continue safely".into(),
        intent: AgentIntent::Implement,
    });
    let mut answer = String::new();
    loop {
        match daemon.next() {
            Event::TextDelta { id: 73, delta } => answer.push_str(&delta),
            Event::Settled { id: 73 } => break,
            _ => {}
        }
    }
    assert_eq!(answer, "continued after cancellation");
    assert_eq!(
        fs::read_to_string(root.join("prompts.jsonl"))
            .expect("read captured prompts")
            .lines()
            .count(),
        2
    );
    fs::remove_dir_all(root).expect("remove fixture");
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
            source: EvidenceSource::LiteralMatch,
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
    for index in 0..MAX_FILES {
        fs::write(root.join(format!("candidate-{index}.rs")), "unrelated\n")
            .expect("write cancellation fixture");
    }
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
    assert_eq!(evidence.source, EvidenceSource::Selection);
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
        intent: AgentIntent::Implement,
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
    assert!(arguments.contains("--tools read,grep,find,ls,edit,write,bash"));
    assert!(!arguments.contains("--no-tools"));
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
fn sends_the_complete_bounded_git_hunk_to_pi() {
    let root = fixture("git-review-repository", "fn changed() {}\n");
    let model_workdir = fixture("git-review-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stream");
    daemon.initialize();
    daemon.trust_model(&root);
    fs::remove_file(root.join("sample.rs")).expect("delete reviewed file");
    daemon.send(&Request::AskAgentSelection {
        id: 31,
        repository: root.clone(),
        query: "Why did this change?".into(),
        intent: AgentIntent::Implement,
        selection: SelectionContext {
            relative_path: "sample.rs".into(),
            language: Some("git-diff".into()),
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
            text: "Git review state: modified\nGit review evidence (untrusted):\n@@ -1 +1 @@\n-old\n+fn changed() {}".into(),
            document_modified: false,
        },
    });
    loop {
        if matches!(daemon.next(), Event::Completed { id: 31, .. }) {
            break;
        }
    }
    let prompt = fs::read_to_string(model_workdir.join("prompt.json")).unwrap();
    assert!(prompt.contains("Git review state: modified"));
    assert!(prompt.contains("@@ -1 +1 @@"));
    assert!(prompt.contains("+fn changed() {}"));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn streams_bounded_typed_pi_tool_activity() {
    use lantern_protocol::WorkbenchTool;

    let root = fixture("pi-tools-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-tools-driver", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "tools");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 34,
        repository: root.clone(),
        query: "Inspect and update this".into(),
        intent: AgentIntent::Implement,
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

    let mut events = Vec::new();
    loop {
        let event = daemon.next();
        if matches!(event, Event::Settled { id: 34 }) {
            break;
        }
        events.push(event);
    }
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ToolStarted {
            id: 34,
            tool: WorkbenchTool::Read,
            relative_path: Some(path),
        } if path == Path::new("sample.rs")
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ToolFinished {
            id: 34,
            tool: WorkbenchTool::Edit,
            success: true,
            ..
        }
    )));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn repository_question_reaches_pi_without_editor_context() {
    let root = fixture("pi-repository-question", "fn selected() {}\n");
    let driver = fixture("pi-repository-question-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "investigation");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgent {
        id: 35,
        repository: root.clone(),
        query: "What does this project do?".into(),
        intent: AgentIntent::Understand,
    });

    let mut search_only_visible = false;
    loop {
        match daemon.next() {
            Event::GroundingState {
                id: 35,
                state: GroundingState::RepositorySearchOnly,
            } => search_only_visible = true,
            Event::Settled { id: 35 } => break,
            _ => {}
        }
    }
    assert!(search_only_visible);
    let prompt = fs::read_to_string(driver.join("prompt.json")).expect("read Pi prompt");
    assert!(prompt.contains("No editor selection was supplied"));
    assert!(prompt.contains("What does this project do?"));
    assert!(!prompt.contains("Selected source"));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn investigation_uses_a_read_only_pi_profile_and_structured_brief_prompt() {
    let root = fixture("pi-investigation", "fn existing_flow() {}\n");
    let driver = fixture("pi-investigation-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let before = fs::read(root.join("sample.rs")).expect("read fixture before investigation");
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "investigation");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgent {
        id: 37,
        repository: root.clone(),
        query: "Add per-workbench model selection".into(),
        intent: AgentIntent::Investigate,
    });

    let mut completed = false;
    let mut evidence = false;
    loop {
        match daemon.next() {
            Event::ToolStarted {
                id: 37,
                tool: WorkbenchTool::Edit | WorkbenchTool::Write | WorkbenchTool::Bash,
                ..
            } => panic!("investigation exposed a mutating tool"),
            Event::Completed { id: 37, .. } => completed = true,
            Event::Evidence {
                id: 37,
                evidence:
                    Evidence {
                        source: EvidenceSource::Investigation,
                        relative_path,
                        ..
                    },
            } => evidence = relative_path == Path::new("sample.rs"),
            Event::Settled { id: 37 } => break,
            _ => {}
        }
    }

    let invocation = fs::read_to_string(driver.join("invocation.args")).expect("read Pi args");
    assert!(invocation.contains("--tools read,grep,find,ls"));
    assert!(!invocation.contains("read,grep,find,ls,edit,write,bash"));
    let prompt = fs::read_to_string(driver.join("prompt.json")).expect("read Pi prompt");
    for heading in [
        "Goal",
        "Observed",
        "Affected flow",
        "Open questions",
        "Acceptance criteria",
        "Readiness",
    ] {
        assert!(prompt.contains(heading), "prompt omitted {heading}");
    }
    assert!(prompt.contains("Do not implement anything"));
    assert!(completed);
    assert!(
        evidence,
        "investigation read must become navigable evidence"
    );
    assert_eq!(
        fs::read(root.join("sample.rs")).expect("read fixture after investigation"),
        before
    );

    daemon.send(&Request::AskAgent {
        id: 38,
        repository: root.clone(),
        query: "Proceed with the smallest implementation.".into(),
        intent: AgentIntent::Implement,
    });
    while !matches!(daemon.next(), Event::Settled { id: 38 }) {}
    let follow_up = fs::read_to_string(driver.join("prompt.json")).expect("read follow-up prompt");
    assert!(follow_up.contains("Prior read-only investigation"));
    assert!(follow_up.contains("Read existing flow"));
    assert!(follow_up.contains("Proceed with the smallest implementation"));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn completed_plan_is_saved_once_without_a_second_model_turn() {
    let root = fixture("persist-plan", "fn existing_flow() {}\n");
    let driver = fixture("persist-plan-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "plan");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgent {
        id: 39,
        repository: root.clone(),
        query: "Turn this into a plan".into(),
        intent: AgentIntent::Plan,
    });
    while !matches!(daemon.next(), Event::Settled { id: 39 }) {}

    daemon.send(&Request::AskAgent {
        id: 40,
        repository: root.clone(),
        query: "Write this down".into(),
        intent: AgentIntent::PersistPlan,
    });
    let mut saved = None;
    loop {
        match daemon.next() {
            Event::PlanSaved {
                id: 40,
                relative_path,
            } => saved = Some(relative_path),
            Event::Settled { id: 40 } => break,
            _ => {}
        }
    }
    assert_eq!(
        saved.as_deref(),
        Some(Path::new(".lantern/plans/active.md"))
    );
    let plan_path = root.join(".lantern/plans/active.md");
    let plan = fs::read_to_string(&plan_path).expect("read saved plan");
    assert!(plan.starts_with(
        "---\nlantern_plan: 1\nstatus: active\n---\n\n# Active implementation plan\n\nObjective\n"
    ));
    let captured_prompt =
        fs::read_to_string(driver.join("prompt.json")).expect("read the only model prompt");
    assert!(captured_prompt.contains("Turn this into a plan"));
    assert!(!captured_prompt.contains("Write this down"));
    let before = fs::read(&plan_path).expect("read plan before duplicate save");

    daemon.send(&Request::AskAgent {
        id: 41,
        repository: root.clone(),
        query: "Save this plan".into(),
        intent: AgentIntent::PersistPlan,
    });
    let mut rejected = false;
    loop {
        match daemon.next() {
            Event::Error {
                id: Some(41),
                message,
                ..
            } => rejected = message.contains("cannot create .lantern/plans/active.md"),
            Event::Settled { id: 41 } => break,
            _ => {}
        }
    }
    assert!(rejected, "duplicate save must retain the existing plan");
    assert_eq!(fs::read(&plan_path).expect("read retained plan"), before);

    let edited = plan.replace(
        "Persist the accepted plan.",
        "Persist the developer-edited plan.",
    );
    fs::write(&plan_path, edited).expect("edit the plan as the developer");
    daemon.send(&Request::AskAgent {
        id: 42,
        repository: root.clone(),
        query: "Proceed with the first task".into(),
        intent: AgentIntent::Implement,
    });
    while !matches!(daemon.next(), Event::Settled { id: 42 }) {}
    let implementation_prompt =
        fs::read_to_string(driver.join("prompt.json")).expect("read implementation prompt");
    assert!(implementation_prompt.contains("Current developer-editable plan"));
    assert!(implementation_prompt.contains("Persist the developer-edited plan"));
    assert!(!implementation_prompt.contains("Persist the accepted plan."));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn multiple_plan_comments_stage_one_revision_before_explicit_application() {
    let root = fixture("plan-review", "fn existing_flow() {}\n");
    let driver = fixture("plan-review-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let plan = "---\nlantern_plan: 1\nstatus: active\n---\n\n# Active implementation plan\n\nObjective\nOriginal objective.\nRepository evidence\n- src/lib.rs:1\nAcceptance criteria\n- One check.\nExclusions\n- No dashboard.\nDecisions\n- One path.\nTasks\n- First task.\n- Second task.\nRisks and unknowns\n- One risk.\nVerification\n- One test.\n";
    let plan_path = root.join(".lantern/plans/active.md");
    fs::create_dir_all(plan_path.parent().unwrap()).expect("create plan directory");
    fs::write(&plan_path, plan).expect("write active plan");
    let anchored = |text: &str| {
        let offset = plan.find(text).expect("anchor in plan");
        let line = plan[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1;
        SelectionContext {
            relative_path: ".lantern/plans/active.md".into(),
            language: Some("markdown".into()),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: text.chars().count() + 1,
            text: text.into(),
            document_modified: false,
        }
    };
    let comments = vec![
        PlanReviewComment {
            anchor: anchored("Original objective."),
            comment: "Make this measurable.".into(),
        },
        PlanReviewComment {
            anchor: anchored("- Second task."),
            comment: "Move this outside the first release.".into(),
        },
    ];
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "plan-review");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::ReviewPlan {
        id: 43,
        repository: root.clone(),
        comments,
    });
    let mut proposal = None;
    loop {
        match daemon.next() {
            Event::ToolStarted {
                id: 43,
                tool: WorkbenchTool::Edit | WorkbenchTool::Write | WorkbenchTool::Bash,
                ..
            } => panic!("plan review exposed a mutating tool"),
            Event::ChangeProposal {
                id: 43,
                proposal: staged,
            } => proposal = Some(staged),
            Event::Settled { id: 43 } => break,
            _ => {}
        }
    }
    let proposal = proposal.expect("one staged plan revision");
    assert!(
        proposal
            .replacement
            .contains("Revised measurable objective")
    );
    assert_eq!(fs::read_to_string(&plan_path).unwrap(), plan);
    let prompt = fs::read_to_string(driver.join("prompt.json")).expect("read review prompt");
    assert!(prompt.contains("Make this measurable"));
    assert!(prompt.contains("Move this outside the first release"));

    daemon.send(&Request::AskAgent {
        id: 44,
        repository: root.clone(),
        query: "Apply that".into(),
        intent: AgentIntent::ApplyPlanRevision,
    });
    let mut applied = false;
    loop {
        match daemon.next() {
            Event::PlanRevisionApplied { id: 44, .. } => applied = true,
            Event::Settled { id: 44 } => break,
            _ => {}
        }
    }
    assert!(applied);
    assert!(
        fs::read_to_string(&plan_path)
            .unwrap()
            .contains("Revised measurable objective")
    );
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn successful_implementation_stages_a_separate_plan_checkpoint() {
    let root = fixture("plan-progress", "fn value() -> u8 { 1 }\n");
    for arguments in [
        &["init", "-q"][..],
        &["config", "user.name", "Lantern Test"],
        &["config", "user.email", "lantern@example.invalid"],
        &["add", "sample.rs"],
        &["commit", "-qm", "fixture"],
    ] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&root)
                .status()
                .expect("run fixture git")
                .success()
        );
    }
    let plan = "---\nlantern_plan: 1\nstatus: active\n---\n\n# Active implementation plan\n\nObjective\nChange the value.\nRepository evidence\n- sample.rs:1\nAcceptance criteria\n- Value returns two.\nExclusions\n- No unrelated changes.\nDecisions\n- Keep the function small.\nTasks\n- Change the value.\nRisks and unknowns\n- Verification pending.\nVerification\n- Run the focused check.\n";
    let plan_path = root.join(".lantern/plans/active.md");
    fs::create_dir_all(plan_path.parent().unwrap()).expect("create plan directory");
    fs::write(&plan_path, plan).expect("write active plan");
    let driver = fixture("plan-progress-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "plan-progress");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgent {
        id: 45,
        repository: root.clone(),
        query: "Proceed with the first task".into(),
        intent: AgentIntent::Implement,
    });
    let mut started = false;
    let mut proposal = None;
    loop {
        match daemon.next() {
            Event::PlanProgressStarted { id: 45 } => started = true,
            Event::ChangeProposal {
                id: 45,
                proposal: staged,
            } => proposal = Some(staged),
            Event::Settled { id: 45 } => break,
            _ => {}
        }
    }
    assert!(started);
    let proposal = proposal.expect("staged plan checkpoint");
    assert!(proposal.replacement.contains("[x] Change the value"));
    assert_eq!(fs::read_to_string(&plan_path).unwrap(), plan);
    let prompt =
        fs::read_to_string(driver.join("progress-prompt.json")).expect("read plan progress prompt");
    assert!(prompt.contains("fn value() -> u8 { 2 }"));
    assert!(prompt.contains("Changed the value and verified it"));
    assert!(prompt.contains("development command completed successfully during the turn: true"));
    assert!(prompt.contains("does not by itself prove verification"));

    daemon.send(&Request::AskAgent {
        id: 46,
        repository: root.clone(),
        query: "Apply that".into(),
        intent: AgentIntent::ApplyPlanRevision,
    });
    while !matches!(daemon.next(), Event::Settled { id: 46 }) {}
    assert!(
        fs::read_to_string(&plan_path)
            .unwrap()
            .contains("[x] Change the value")
    );
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn submitted_line_comments_drive_one_coherent_correction_turn() {
    let root = fixture("code-review", "fn value() -> u8 { 1 }\n");
    for arguments in [
        &["init", "-q"][..],
        &["config", "user.name", "Lantern Test"],
        &["config", "user.email", "lantern@example.invalid"],
        &["add", "sample.rs"],
        &["commit", "-qm", "fixture"],
    ] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&root)
                .status()
                .expect("run fixture git")
                .success()
        );
    }
    fs::write(root.join("sample.rs"), "fn value() -> u8 { 2 }\n").expect("write first revision");
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff", "--unified=3", "--", "sample.rs"])
        .current_dir(&root)
        .output()
        .expect("read review diff");
    let full_diff = String::from_utf8(output.stdout).expect("UTF-8 review diff");
    let hunk = full_diff[full_diff.find("@@").expect("diff hunk")..].to_owned();
    let review = GitReviewContext {
        relative_path: "sample.rs".into(),
        state: GitReviewState::Modified,
        scope: GitReviewScope::Hunk,
        start_line: 1,
        end_line: 2,
        diff: hunk.clone(),
    };
    let anchored = |needle: &str, comment: &str| {
        let diff_line = hunk
            .lines()
            .position(|line| line.contains(needle))
            .expect("reviewed line");
        CodeReviewComment {
            anchor: CodeReviewAnchor {
                review: review.clone(),
                diff_line,
                line: hunk.lines().nth(diff_line).unwrap().into(),
            },
            comment: comment.into(),
        }
    };
    let comments = vec![
        anchored("{ 2 }", "Return three instead."),
        anchored("{ 1 }", "Keep this function signature unchanged."),
    ];
    let driver = fixture("code-review-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "code-review");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::ReviewCode {
        id: 47,
        repository: root.clone(),
        comments,
    });
    let mut completed = false;
    loop {
        match daemon.next() {
            Event::Completed { id: 47, .. } => completed = true,
            Event::Settled { id: 47 } => break,
            _ => {}
        }
    }
    assert!(completed);
    assert_eq!(
        fs::read_to_string(root.join("sample.rs")).unwrap(),
        "fn value() -> u8 { 3 }\n"
    );
    let prompt = fs::read_to_string(driver.join("code-review-prompt.json"))
        .expect("read code review prompt");
    assert!(prompt.contains("Return three instead"));
    assert!(prompt.contains("Keep this function signature unchanged"));
    assert!(prompt.contains("Address every developer comment as one coherent correction"));
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
}

#[cfg(unix)]
#[test]
fn external_repository_journey_edits_tests_and_leaves_a_reviewable_diff() {
    use lantern_protocol::WorkbenchTool;

    let root = fixture(
        "external-edit-journey",
        "pub fn greeting() -> &'static str { \"old\" }\n",
    );
    let test_path = root.join("focused-test.sh");
    fs::write(
        &test_path,
        "#!/usr/bin/env bash\nset -euo pipefail\ngrep -q '\"new\"' sample.rs\n",
    )
    .expect("write focused test");
    let mut permissions = fs::metadata(&test_path)
        .expect("test metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&test_path, permissions).expect("make focused test executable");
    for arguments in [
        vec!["init", "-q"],
        vec!["add", "sample.rs", "focused-test.sh"],
        vec![
            "-c",
            "user.name=Lantern Test",
            "-c",
            "user.email=lantern@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "baseline",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(&root)
                .status()
                .expect("run Git fixture command")
                .success()
        );
    }

    let driver = fixture("external-edit-journey-driver", "private\n");
    let pi_bin = fake_pi(&driver);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &driver, "journey");
    daemon.initialize();
    daemon.open(&root);
    daemon.send(&Request::AskAgent {
        id: 36,
        repository: root.clone(),
        query: "Change the greeting from old to new and run the focused test.".into(),
        intent: AgentIntent::Implement,
    });

    let mut tools = Vec::new();
    let mut answer = String::new();
    let mut completed = false;
    loop {
        match daemon.next() {
            Event::ToolStarted { id: 36, tool, .. } => tools.push(tool),
            Event::ToolFinished {
                id: 36,
                success: false,
                ..
            } => panic!("journey tool failed"),
            Event::TextDelta { id: 36, delta } => answer.push_str(&delta),
            Event::Completed { id: 36, .. } => completed = true,
            Event::Settled { id: 36 } => break,
            _ => {}
        }
    }

    assert_eq!(
        tools,
        [
            WorkbenchTool::Read,
            WorkbenchTool::Edit,
            WorkbenchTool::Bash
        ]
    );
    assert!(completed, "journey must complete before settlement");
    assert!(answer.contains("verified"));
    assert_eq!(
        fs::read_to_string(root.join("sample.rs")).expect("read changed file"),
        "pub fn greeting() -> &str { \"new\" }\n"
    );
    let diff = Command::new("git")
        .args(["diff", "--", "sample.rs"])
        .current_dir(&root)
        .output()
        .expect("read journey diff");
    assert!(diff.status.success());
    let diff = String::from_utf8(diff.stdout).expect("UTF-8 diff");
    assert!(diff.contains("-pub fn greeting() -> &'static str { \"old\" }"));
    assert!(diff.contains("+pub fn greeting() -> &str { \"new\" }"));
    assert!(
        !driver.join("thinking.jsonl").exists(),
        "multi-step repository work must retain the configured medium reasoning level"
    );

    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(driver).expect("remove driver fixture");
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
        intent: AgentIntent::Implement,
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
fn provider_stderr_is_not_copied_into_user_visible_errors() {
    let root = fixture("pi-stderr-private-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-stderr-private-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stderr-close");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 34,
        repository: root.clone(),
        query: "Explain this".into(),
        intent: AgentIntent::Implement,
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

    let message = loop {
        if let Event::Error {
            id: Some(34),
            message,
            ..
        } = daemon.next()
        {
            break message;
        }
    };
    assert!(message.contains("provider stderr was excluded"));
    assert!(!message.contains("sk-provider-secret"));
    while !matches!(daemon.next(), Event::Settled { id: 34 }) {}
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn provider_rejection_detail_is_not_copied_into_user_visible_errors() {
    let root = fixture("pi-rejection-private-repository", "fn selected() {}\n");
    let model_workdir = fixture("pi-rejection-private-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "rejected");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSelection {
        id: 35,
        repository: root.clone(),
        query: "Explain this".into(),
        intent: AgentIntent::Implement,
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

    let (message, recovery) = loop {
        if let Event::Error {
            id: Some(35),
            message,
            recovery,
        } = daemon.next()
        {
            break (message, recovery);
        }
    };
    assert_eq!(
        message,
        "Pi rejected the request; provider detail was excluded"
    );
    assert!(!message.contains("sk-provider-response-secret"));
    assert!(recovery.contains("inspect provider status"));
    assert!(recovery.contains("use `/login` for OpenAI Codex if required"));
    while !matches!(daemon.next(), Event::Settled { id: 35 }) {}
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
        intent: AgentIntent::Implement,
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
    daemon.send(&Request::AskAgent {
        id: 29,
        repository: root.clone(),
        query: "Do not restart silently".into(),
        intent: AgentIntent::Implement,
    });
    let mut failed_visibly = false;
    loop {
        match daemon.next() {
            Event::Error {
                id: Some(29),
                message,
                ..
            } if message.contains("cannot send Pi command") => failed_visibly = true,
            Event::Settled { id: 29 } => break,
            _ => {}
        }
    }
    assert!(
        failed_visibly,
        "dead persistent Pi driver was silently replaced"
    );
    assert_eq!(
        fs::read_to_string(model_workdir.join("pi.pid")).expect("read retained Pi pid"),
        pid
    );
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn streams_definition_and_references_before_a_symbol_grounded_answer() {
    let root = fixture("symbol-repository", "fn caller() { resolved(); }\n");
    let definition = (1..=16)
        .map(|line| {
            if line == 1 {
                "fn resolved() {}".into()
            } else if line == 16 {
                "// bounded-definition-tail".into()
            } else {
                format!("// definition context {line}")
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    fs::write(root.join("definition.rs"), format!("{definition}\n")).expect("write definition");
    let call = (1..=32)
        .map(|line| {
            if line == 1 {
                "fn dispatch() {}".into()
            } else if line == 32 {
                "// bounded-call-tail".into()
            } else {
                format!("// call context {line}")
            }
        })
        .collect::<Vec<String>>()
        .join("\n");
    fs::write(root.join("call.rs"), format!("{call}\n")).expect("write call evidence");
    let model_workdir = fixture("symbol-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "stream");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSymbol {
        id: 13,
        repository: root.clone(),
        query: "Where is this defined and used?".into(),
        intent: AgentIntent::Implement,
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
            calls: vec![SymbolCall {
                name: "dispatch".into(),
                depth: 2,
                location: SymbolLocation {
                    relative_path: "call.rs".into(),
                    start_line: 1,
                    start_column: 4,
                    end_line: 1,
                    end_column: 12,
                },
            }],
        },
    });

    let mut evidence_records = Vec::new();
    loop {
        match daemon.next() {
            Event::Evidence { id: 13, evidence } => {
                evidence_records.push((evidence.source, evidence.relative_path))
            }
            Event::Completed {
                id: 13,
                evidence_count,
            } => {
                assert_eq!(evidence_count, 4);
                break;
            }
            _ => {}
        }
    }
    assert_eq!(
        evidence_records,
        vec![
            (EvidenceSource::Selection, PathBuf::from("sample.rs")),
            (EvidenceSource::Call, PathBuf::from("call.rs")),
            (EvidenceSource::Definition, PathBuf::from("definition.rs")),
            (EvidenceSource::Reference, PathBuf::from("sample.rs")),
        ]
    );
    let prompt = fs::read_to_string(model_workdir.join("prompt.json")).unwrap();
    assert!(prompt.contains("<definition path=\\\"definition.rs\\\""));
    assert!(prompt.contains("<reference path=\\\"sample.rs\\\""));
    assert!(prompt.contains("fn caller() { resolved(); }"));
    assert!(prompt.contains("fn resolved() {}"));
    assert!(prompt.contains("bounded-definition-tail"));
    assert!(prompt.contains("Name (untrusted): \\\"dispatch\\\""));
    assert!(prompt.contains("bounded-call-tail"));
    assert!(prompt.contains("Answer directly without tools"));
    assert_eq!(await_thinking_levels(&model_workdir, 2), ["off", "medium"]);
    fs::remove_dir_all(root).expect("remove repository fixture");
    fs::remove_dir_all(model_workdir).expect("remove model fixture");
}

#[cfg(unix)]
#[test]
fn symbol_reasoning_starts_without_reasoning_and_escalates_before_tool_results() {
    let root = fixture(
        "reasoning-escalation-repository",
        "fn caller() { resolved(); }\n",
    );
    fs::write(root.join("definition.rs"), "fn resolved() {}\n").expect("write definition");
    let model_workdir = fixture("reasoning-escalation-workdir", "private\n");
    let pi_bin = fake_pi(&model_workdir);
    let mut daemon = Daemon::spawn_with_pi(&pi_bin, &model_workdir, "reasoning-escalation");
    daemon.initialize();
    daemon.trust_model(&root);
    daemon.send(&Request::AskAgentSymbol {
        id: 74,
        repository: root.clone(),
        query: "Follow the missing handoff".into(),
        intent: AgentIntent::Implement,
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
            references: vec![],
            calls: vec![],
        },
    });
    while !matches!(daemon.next(), Event::Settled { id: 74 }) {}

    assert_eq!(
        await_thinking_levels(&model_workdir, 3),
        ["off", "medium", "medium"]
    );
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
        intent: AgentIntent::Implement,
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
