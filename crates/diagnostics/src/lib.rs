use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;
pub const MAX_BUNDLE_RECORDS: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    Daemon,
    Policy,
    Protocol,
    Provider,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    DaemonStarted,
    DaemonStopping,
    ProtocolInitialized,
    ProtocolRejected,
    WorkspaceConfigured,
    WorkspaceRejected,
    OperationAccepted,
    RequestFailed,
    OperationSettled,
    ProviderFailed,
}

impl Code {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DaemonStarted => "daemon started",
            Self::DaemonStopping => "daemon stopping",
            Self::ProtocolInitialized => "protocol initialized",
            Self::ProtocolRejected => "protocol rejected",
            Self::WorkspaceConfigured => "workspace configured",
            Self::WorkspaceRejected => "workspace rejected",
            Self::OperationAccepted => "operation accepted",
            Self::RequestFailed => "request failed",
            Self::OperationSettled => "operation settled",
            Self::ProviderFailed => "provider failed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema_version: u32,
    pub timestamp_ms: u64,
    pub level: Level,
    pub component: Component,
    pub code: Code,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<u64>,
}

impl Record {
    pub fn new(level: Level, component: Component, code: Code) -> Self {
        Self {
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            timestamp_ms: unix_millis(),
            level,
            component,
            code,
            operation_id: None,
        }
    }

    pub const fn for_operation(mut self, operation_id: u64) -> Self {
        self.operation_id = Some(operation_id);
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Starting,
    Ready,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub schema_version: u32,
    pub generated_at_ms: u64,
    pub lantern_version: String,
    pub protocol_version: u32,
    pub platform: Platform,
    pub daemon_state: DaemonState,
    pub records: Vec<Record>,
    pub ignored_unstructured_lines: usize,
}

pub fn emit(record: &Record) -> io::Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    serde_json::to_writer(&mut stderr, record).map_err(io::Error::other)?;
    stderr.write_all(b"\n")?;
    stderr.flush()
}

pub fn bundle_from_stderr(
    stderr: &str,
    lantern_version: &str,
    protocol_version: u32,
    daemon_state: DaemonState,
) -> Bundle {
    let (records, ignored_unstructured_lines) = parse_stderr(stderr);
    Bundle {
        schema_version: DIAGNOSTIC_SCHEMA_VERSION,
        generated_at_ms: unix_millis(),
        lantern_version: lantern_version.to_owned(),
        protocol_version,
        platform: Platform {
            os: std::env::consts::OS.to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
        },
        daemon_state,
        records,
        ignored_unstructured_lines,
    }
}

pub fn summarize_stderr(stderr: &str) -> String {
    let (records, ignored) = parse_stderr(stderr);
    let ignored = if ignored == 0 {
        String::new()
    } else {
        format!("; {ignored} unstructured line(s) excluded")
    };
    match records.last() {
        Some(latest) => {
            let operation = latest
                .operation_id
                .map_or_else(String::new, |id| format!(" for operation {id}"));
            format!(
                "{} structured event(s); latest: {}{}{ignored}",
                records.len(),
                latest.code.label(),
                operation,
            )
        }
        None if ignored.is_empty() => String::new(),
        None => format!("No structured diagnostics available{ignored}"),
    }
}

fn parse_stderr(stderr: &str) -> (Vec<Record>, usize) {
    let mut records = Vec::new();
    let mut ignored = 0;
    for line in stderr.lines() {
        match serde_json::from_str::<Record>(line) {
            Ok(record) if record.schema_version == DIAGNOSTIC_SCHEMA_VERSION => {
                records.push(record);
                if records.len() > MAX_BUNDLE_RECORDS {
                    records.remove(0);
                }
            }
            _ => ignored += 1,
        }
    }
    (records, ignored)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_discards_unstructured_stderr_instead_of_redacting_by_guesswork() {
        let record = Record::new(Level::Error, Component::Provider, Code::ProviderFailed);
        let structured = serde_json::to_string(&record).expect("serialize record");
        let stderr = format!("provider leaked sk-sensitive-value\n{structured}\n");
        let bundle = bundle_from_stderr(&stderr, "0.1.0", 3, DaemonState::Unavailable);
        let exported = serde_json::to_string(&bundle).expect("serialize bundle");

        assert_eq!(bundle.records, [record]);
        assert_eq!(bundle.ignored_unstructured_lines, 1);
        assert!(!exported.contains("sk-sensitive-value"));
    }

    #[test]
    fn bundle_retains_only_the_latest_bounded_records() {
        let stderr = (0..MAX_BUNDLE_RECORDS + 2)
            .map(|id| {
                serde_json::to_string(
                    &Record::new(Level::Info, Component::Daemon, Code::OperationSettled)
                        .for_operation(id as u64),
                )
                .expect("serialize record")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let bundle = bundle_from_stderr(&stderr, "0.1.0", 3, DaemonState::Ready);

        assert_eq!(bundle.records.len(), MAX_BUNDLE_RECORDS);
        assert_eq!(
            bundle
                .records
                .first()
                .and_then(|record| record.operation_id),
            Some(2)
        );
    }

    #[test]
    fn record_schema_rejects_arbitrary_fields() {
        let json = r#"{"schema_version":1,"timestamp_ms":1,"level":"info","component":"daemon","code":"daemon_started","message":"source body"}"#;
        assert!(serde_json::from_str::<Record>(json).is_err());
    }

    #[test]
    fn summary_never_echoes_unstructured_stderr() {
        let summary = summarize_stderr("password=secret-value\n");
        assert_eq!(
            summary,
            "No structured diagnostics available; 1 unstructured line(s) excluded"
        );
        assert!(!summary.contains("secret-value"));
    }
}
