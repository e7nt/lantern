use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self, BufRead};
use std::path::{Component, Path, PathBuf};

pub const PROTOCOL_VERSION: u32 = 6;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 256 * 1024;
pub const MAX_DIAGNOSTIC_BYTES: usize = 8 * 1024;
pub const MAX_FILES: usize = 2_000;
pub const MAX_FILE_BYTES: u64 = 512 * 1024;
pub const MAX_EVIDENCE: usize = 5;
pub const MAX_SELECTION_BYTES: usize = 64 * 1024;
pub const MAX_SYMBOL_REFERENCES: usize = 8;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectionContext {
    pub relative_path: PathBuf,
    pub language: Option<String>,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub text: String,
    pub document_modified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolLocation {
    pub relative_path: PathBuf,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolContext {
    pub selection: SelectionContext,
    pub definition: SymbolLocation,
    pub references: Vec<SymbolLocation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SymbolContextExport {
    Resolved {
        selection: SelectionContext,
        definition: SymbolLocation,
        references: Vec<SymbolLocation>,
    },
    Error {
        message: String,
    },
}

impl SymbolContextExport {
    pub fn into_context(self) -> Result<SymbolContext, String> {
        match self {
            Self::Resolved {
                selection,
                definition,
                references,
            } => Ok(SymbolContext {
                selection,
                definition,
                references,
            }),
            Self::Error { message } => Err(format!("LSP symbol context failed: {message}")),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Initialize {
        protocol_version: u32,
    },
    OpenWorkbench {
        repository: PathBuf,
    },
    Ask {
        id: u64,
        repository: PathBuf,
        query: String,
    },
    AskSelection {
        id: u64,
        repository: PathBuf,
        query: String,
        selection: SelectionContext,
    },
    AskAgentSelection {
        id: u64,
        repository: PathBuf,
        query: String,
        selection: SelectionContext,
    },
    AskAgent {
        id: u64,
        repository: PathBuf,
        query: String,
    },
    AskAgentSymbol {
        id: u64,
        repository: PathBuf,
        query: String,
        context: SymbolContext,
    },
    PreviewSelection {
        id: u64,
        repository: PathBuf,
        selection: SelectionContext,
        replacement: String,
    },
    Cancel {
        id: u64,
    },
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: EvidenceSource,
    pub relative_path: PathBuf,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub excerpt: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    Selection,
    Definition,
    Reference,
    LiteralMatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchTool {
    Read,
    Grep,
    Find,
    List,
    Edit,
    Write,
    Bash,
}

impl WorkbenchTool {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Read => "Reading code",
            Self::Grep => "Searching code",
            Self::Find => "Finding files",
            Self::List => "Listing files",
            Self::Edit => "Editing code",
            Self::Write => "Writing code",
            Self::Bash => "Running a development command",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeProposal {
    pub selection: SelectionContext,
    pub replacement: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Initialized {
        protocol_version: u32,
    },
    WorkbenchOpened {
        repository: PathBuf,
    },
    WorkbenchOpenFailed {
        message: String,
        recovery: String,
    },
    Accepted {
        id: u64,
    },
    OperationStarted {
        id: u64,
        search_term: String,
    },
    Progress {
        id: u64,
        files_inspected: usize,
    },
    Evidence {
        id: u64,
        evidence: Evidence,
    },
    ChangeProposal {
        id: u64,
        proposal: ChangeProposal,
    },
    TextDelta {
        id: u64,
        delta: String,
    },
    ToolStarted {
        id: u64,
        tool: WorkbenchTool,
        relative_path: Option<PathBuf>,
    },
    ToolFinished {
        id: u64,
        tool: WorkbenchTool,
        relative_path: Option<PathBuf>,
        success: bool,
    },
    Completed {
        id: u64,
        evidence_count: usize,
    },
    Cancelled {
        id: u64,
        cancellation_latency_ms: u64,
    },
    Error {
        id: Option<u64>,
        message: String,
        recovery: String,
    },
    Settled {
        id: u64,
    },
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    InvalidUtf8,
    TooLarge { limit: usize },
}

#[derive(Debug)]
pub struct BoundedTail {
    bytes: Vec<u8>,
    capacity: usize,
}

impl BoundedTail {
    pub fn new(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        if self.capacity == 0 {
            return;
        }
        if chunk.len() >= self.capacity {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&chunk[chunk.len() - self.capacity..]);
            return;
        }
        let overflow = self
            .bytes
            .len()
            .saturating_add(chunk.len())
            .saturating_sub(self.capacity);
        if overflow > 0 {
            self.bytes.drain(..overflow);
        }
        self.bytes.extend_from_slice(chunk);
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(cause) => write!(formatter, "cannot read protocol frame: {cause}"),
            Self::InvalidUtf8 => formatter.write_str("protocol frame is not valid UTF-8"),
            Self::TooLarge { limit } => {
                write!(formatter, "protocol frame exceeds the {limit}-byte limit")
            }
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(cause: io::Error) -> Self {
        Self::Io(cause)
    }
}

/// Reads one LF-delimited UTF-8 frame without allowing an unbounded allocation.
/// A trailing CR is accepted so clients may use CRLF. Unicode line separators are content.
pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<String>, FrameError> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                decode_frame(bytes).map(Some)
            };
        }

        if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
            if bytes.len().saturating_add(newline) > MAX_FRAME_BYTES {
                reader.consume(newline + 1);
                return Err(FrameError::TooLarge {
                    limit: MAX_FRAME_BYTES,
                });
            }
            bytes.extend_from_slice(&available[..newline]);
            reader.consume(newline + 1);
            return decode_frame(bytes).map(Some);
        }

        if bytes.len().saturating_add(available.len()) > MAX_FRAME_BYTES {
            let consumed = available.len();
            reader.consume(consumed);
            drain_frame(reader)?;
            return Err(FrameError::TooLarge {
                limit: MAX_FRAME_BYTES,
            });
        }
        bytes.extend_from_slice(available);
        let consumed = available.len();
        reader.consume(consumed);
    }
}

fn drain_frame(reader: &mut impl BufRead) -> Result<(), FrameError> {
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |newline| newline + 1);
        let finished = available.get(consumed.saturating_sub(1)) == Some(&b'\n');
        reader.consume(consumed);
        if finished {
            return Ok(());
        }
    }
}

fn decode_frame(mut bytes: Vec<u8>) -> Result<String, FrameError> {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|_| FrameError::InvalidUtf8)
}

pub fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("evidence path must be a non-empty repository-relative path".into());
    }

    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("evidence path contains a repository escape or unsupported component".into());
    }

    Ok(())
}

pub fn validate_selection(selection: &SelectionContext) -> Result<(), String> {
    validate_relative_path(&selection.relative_path)?;
    if selection.text.is_empty() {
        return Err("selection text is empty".into());
    }
    if selection.text.len() > MAX_SELECTION_BYTES {
        return Err(format!(
            "selection exceeds the {MAX_SELECTION_BYTES}-byte limit"
        ));
    }
    if selection.start_line == 0
        || selection.start_column == 0
        || selection.end_line == 0
        || selection.end_column == 0
        || (selection.end_line, selection.end_column)
            <= (selection.start_line, selection.start_column)
    {
        return Err("selection range must be a non-empty one-based range".into());
    }
    Ok(())
}

pub fn validate_symbol_location(location: &SymbolLocation) -> Result<(), String> {
    validate_relative_path(&location.relative_path)?;
    if location.start_line == 0
        || location.start_column == 0
        || location.end_line == 0
        || location.end_column == 0
        || (location.end_line, location.end_column) <= (location.start_line, location.start_column)
    {
        return Err("symbol location must be a non-empty one-based range".into());
    }
    Ok(())
}

pub fn validate_symbol_context(context: &SymbolContext) -> Result<(), String> {
    validate_selection(&context.selection)?;
    validate_symbol_location(&context.definition)?;
    if context.references.len() > MAX_SYMBOL_REFERENCES {
        return Err(format!(
            "symbol context exceeds the {MAX_SYMBOL_REFERENCES}-reference limit"
        ));
    }
    for reference in &context.references {
        validate_symbol_location(reference)?;
    }
    Ok(())
}

pub fn search_term(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let candidate = trimmed
        .strip_prefix("/show ")
        .or_else(|| trimmed.strip_prefix("show "))
        .unwrap_or(trimmed)
        .trim();

    if candidate.is_empty() {
        return Err("enter `/show <literal text>`".into());
    }
    if candidate.len() > 256 {
        return Err("the search term exceeds the 256-character spike limit".into());
    }

    Ok(candidate.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_explicit_show_term() {
        assert_eq!(search_term("/show AgentDriver").unwrap(), "AgentDriver");
        assert_eq!(search_term("show AgentDriver").unwrap(), "AgentDriver");
    }

    #[test]
    fn rejects_repository_escape() {
        assert!(validate_relative_path(Path::new("../secret")).is_err());
        assert!(validate_relative_path(Path::new("/tmp/secret")).is_err());
        assert!(validate_relative_path(Path::new("src/main.rs")).is_ok());
    }

    #[test]
    fn rejects_empty_or_oversized_selection_context() {
        let mut selection = SelectionContext {
            relative_path: "src/main.rs".into(),
            language: Some("rust".into()),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
            text: String::new(),
            document_modified: false,
        };
        assert!(validate_selection(&selection).is_err());
        selection.text = "x".repeat(MAX_SELECTION_BYTES + 1);
        assert!(validate_selection(&selection).is_err());
    }

    #[test]
    fn rejects_unbounded_symbol_references() {
        let location = SymbolLocation {
            relative_path: "src/lib.rs".into(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
        };
        let context = SymbolContext {
            selection: SelectionContext {
                relative_path: "src/lib.rs".into(),
                language: Some("rust".into()),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 2,
                text: "x".into(),
                document_modified: false,
            },
            definition: location.clone(),
            references: vec![location; MAX_SYMBOL_REFERENCES + 1],
        };
        assert_eq!(
            validate_symbol_context(&context).unwrap_err(),
            "symbol context exceeds the 8-reference limit"
        );
    }

    #[test]
    fn rejects_unknown_evidence_provenance() {
        let event = r#"{"type":"evidence","id":1,"evidence":{"source":"model_guess","relative_path":"src/lib.rs","start_line":1,"start_column":1,"end_line":1,"end_column":2,"excerpt":"x"}}"#;
        assert!(serde_json::from_str::<Event>(event).is_err());
    }

    #[test]
    fn preserves_explicit_lsp_export_errors() {
        let export = SymbolContextExport::Error {
            message: "no repository definition".into(),
        };
        assert_eq!(
            export.into_context().unwrap_err(),
            "LSP symbol context failed: no repository definition"
        );
    }

    #[test]
    fn bounded_tail_retains_only_the_latest_diagnostics() {
        let mut tail = BoundedTail::new(8);
        tail.push(b"abcde");
        tail.push(b"fghijk");
        assert_eq!(tail.text(), "defghijk");
        tail.push(b"0123456789");
        assert_eq!(tail.text(), "23456789");
    }
}
