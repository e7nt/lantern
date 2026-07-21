use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self, BufRead};
use std::path::{Component, Path, PathBuf};

pub const PROTOCOL_VERSION: u32 = 14;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_EVENT_BYTES: usize = 256 * 1024;
pub const MAX_DIAGNOSTIC_BYTES: usize = 8 * 1024;
pub const MAX_FILES: usize = 2_000;
pub const MAX_FILE_BYTES: u64 = 512 * 1024;
pub const MAX_EVIDENCE: usize = 5;
pub const MAX_SELECTION_BYTES: usize = 64 * 1024;
pub const MAX_QUESTION_BYTES: usize = 64 * 1024;
pub const MAX_SYMBOL_REFERENCES: usize = 8;
pub const MAX_SYMBOL_CALLS: usize = 8;
pub const MAX_SYMBOL_NAME_BYTES: usize = 256;
pub const MAX_AGENT_TOUCHED_PATHS: usize = 64;
pub const MAX_AGENT_GIT_FOCUS_BYTES: usize = 16 * 1024;
pub const MAX_PLAN_REVIEW_COMMENTS: usize = 32;
pub const MAX_PLAN_COMMENT_BYTES: usize = 8 * 1024;
pub const MAX_PLAN_REVIEW_BYTES: usize = 64 * 1024;

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GitReviewState {
    Conflict,
    Staged,
    Modified,
    Untracked,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GitReviewScope {
    File,
    Hunk,
}

impl fmt::Display for GitReviewState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Conflict => "conflict",
            Self::Staged => "staged",
            Self::Modified => "modified",
            Self::Untracked => "untracked",
        };
        formatter.write_str(label)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitReviewContext {
    pub relative_path: PathBuf,
    pub state: GitReviewState,
    pub scope: GitReviewScope,
    pub start_line: usize,
    pub end_line: usize,
    pub diff: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentGitFocus {
    pub relative_paths: Vec<PathBuf>,
}

impl GitReviewContext {
    pub fn into_selection(self) -> SelectionContext {
        SelectionContext {
            relative_path: self.relative_path,
            language: Some("git-diff".into()),
            start_line: self.start_line,
            start_column: 1,
            end_line: self.end_line,
            end_column: 1,
            text: format!(
                "Git review state: {}\nGit review scope: {}\nGit review evidence (untrusted):\n{}",
                self.state,
                match self.scope {
                    GitReviewScope::File => "file",
                    GitReviewScope::Hunk => "hunk",
                },
                self.diff
            ),
            document_modified: false,
        }
    }
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
pub struct SymbolCall {
    pub name: String,
    pub depth: u8,
    pub location: SymbolLocation,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolContext {
    pub selection: SelectionContext,
    pub definition: SymbolLocation,
    pub references: Vec<SymbolLocation>,
    pub calls: Vec<SymbolCall>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentIntent {
    Understand,
    Investigate,
    Plan,
    PersistPlan,
    ApplyPlanRevision,
    Implement,
}

pub fn infer_agent_intent(query: &str, previous: Option<AgentIntent>) -> AgentIntent {
    let query = query.trim().to_lowercase();
    let contains_any = |terms: &[&str]| terms.iter().any(|term| query.contains(term));
    let read_only = contains_any(&[
        "don't change",
        "do not change",
        "don't edit",
        "do not edit",
        "don't implement",
        "do not implement",
        "don't add",
        "do not add",
        "don't remove",
        "do not remove",
        "don't write",
        "do not write",
        "without changing",
        "without editing",
        "without modifying",
        "read-only",
        "read only",
    ]);
    let exploratory = contains_any(&[
        "investigate",
        "look into",
        "assess",
        "evaluate",
        "explore",
        "can we ",
        "could we ",
        "should we ",
        "is it possible",
        "how would",
        "what would",
        "would it ",
        "do you think",
    ]);
    let informational = contains_any(&[
        "explain",
        "where ",
        "where is",
        "what is",
        "what does",
        "how does",
        "why ",
        "why does",
        "show me",
    ]);
    if contains_any(&[
        "write this down",
        "save this plan",
        "save the plan",
        "keep this as our plan",
        "persist the plan",
    ]) {
        return AgentIntent::PersistPlan;
    }
    if contains_any(&[
        "apply that",
        "apply the review",
        "apply these comments",
        "address all of them",
        "address these comments",
    ]) {
        return AgentIntent::ApplyPlanRevision;
    }
    if informational && !exploratory {
        return AgentIntent::Understand;
    }
    if read_only || exploratory {
        return AgentIntent::Investigate;
    }
    if contains_any(&[
        "turn this into a plan",
        "make a plan",
        "create a plan",
        "write a plan",
        "plan this",
        "implementation plan",
        "roadmap",
        "break this down",
        "we should ",
    ]) {
        return AgentIntent::Plan;
    }
    if contains_any(&[
        "proceed",
        "implement",
        "fix ",
        "change ",
        "add ",
        "remove ",
        "update ",
        "refactor ",
        "create ",
        "write ",
        "make ",
        "apply ",
        "rename ",
        "move ",
        "replace ",
        "go ahead",
        "do it",
        "let's do",
    ]) {
        return AgentIntent::Implement;
    }
    let continues_previous = query == "yes"
        || query.starts_with("yes, ")
        || query.starts_with("yes but ")
        || query.starts_with("but ")
        || query.starts_with("and ")
        || query.starts_with("also ")
        || query.starts_with("instead ")
        || query.starts_with("only ")
        || query.starts_with("keep ")
        || query.starts_with("not ");
    if continues_previous
        && let Some(intent @ (AgentIntent::Investigate | AgentIntent::Plan)) = previous
    {
        return intent;
    }
    AgentIntent::Understand
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SymbolContextExport {
    Resolved {
        selection: SelectionContext,
        definition: SymbolLocation,
        references: Vec<SymbolLocation>,
        calls: Vec<SymbolCall>,
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
                calls,
            } => Ok(SymbolContext {
                selection,
                definition,
                references,
                calls,
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
        intent: AgentIntent,
    },
    AskAgent {
        id: u64,
        repository: PathBuf,
        query: String,
        intent: AgentIntent,
    },
    AskAgentSymbol {
        id: u64,
        repository: PathBuf,
        query: String,
        context: SymbolContext,
        intent: AgentIntent,
    },
    ReviewPlan {
        id: u64,
        repository: PathBuf,
        comments: Vec<PlanReviewComment>,
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

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlRequest {
    SubmitQuestion { question: String },
    AddPlanComment { comment: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanReviewComment {
    pub anchor: SelectionContext,
    pub comment: String,
}

pub fn validate_plan_review(comments: &[PlanReviewComment]) -> Result<(), String> {
    if comments.is_empty() || comments.len() > MAX_PLAN_REVIEW_COMMENTS {
        return Err(format!(
            "plan review must contain 1 to {MAX_PLAN_REVIEW_COMMENTS} comments"
        ));
    }
    let mut total = 0_usize;
    for item in comments {
        validate_selection(&item.anchor)?;
        if item.anchor.relative_path != Path::new(".lantern/plans/active.md") {
            return Err("plan review comments must anchor to the active plan".into());
        }
        if item.anchor.document_modified {
            return Err("save the active plan before adding review comments".into());
        }
        let comment = item.comment.trim();
        if comment.is_empty() || comment.len() > MAX_PLAN_COMMENT_BYTES {
            return Err(format!(
                "each plan comment must contain 1 to {MAX_PLAN_COMMENT_BYTES} bytes"
            ));
        }
        total = total
            .checked_add(item.anchor.text.len())
            .and_then(|size| size.checked_add(comment.len()))
            .ok_or("plan review size overflow")?;
    }
    if total > MAX_PLAN_REVIEW_BYTES {
        return Err(format!(
            "plan review exceeds the {MAX_PLAN_REVIEW_BYTES} byte limit"
        ));
    }
    Ok(())
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
    Call,
    Semantic,
    LiteralMatch,
    Investigation,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroundingState {
    PreparingIndex,
    RepositorySearchOnly,
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
    GroundingState {
        id: u64,
        state: GroundingState,
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
    PlanSaved {
        id: u64,
        relative_path: PathBuf,
    },
    PlanRevisionApplied {
        id: u64,
        relative_path: PathBuf,
    },
    PlanProgressStarted {
        id: u64,
    },
    PlanProgressFailed {
        id: u64,
        message: String,
        recovery: String,
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

pub fn validate_git_review(context: &GitReviewContext) -> Result<(), String> {
    validate_relative_path(&context.relative_path)?;
    if context.start_line == 0 || context.end_line <= context.start_line {
        return Err("Git review range must be a non-empty one-based range".into());
    }
    if context.diff.is_empty() {
        return Err("Git review diff is empty".into());
    }
    let envelope_bytes =
        "Git review state: \nGit review scope: hunk\nGit review evidence (untrusted):\n".len()
            + context.state.to_string().len();
    if context.diff.len() + envelope_bytes > MAX_SELECTION_BYTES {
        return Err(format!(
            "Git review exceeds the {MAX_SELECTION_BYTES}-byte selection limit"
        ));
    }
    Ok(())
}

pub fn validate_agent_git_focus(focus: &AgentGitFocus) -> Result<(), String> {
    if focus.relative_paths.is_empty() {
        return Err("agent Git focus contains no paths".into());
    }
    if focus.relative_paths.len() > MAX_AGENT_TOUCHED_PATHS {
        return Err(format!(
            "agent Git focus exceeds the {MAX_AGENT_TOUCHED_PATHS}-path limit"
        ));
    }
    let mut unique = std::collections::HashSet::new();
    let mut path_bytes = 0;
    for path in &focus.relative_paths {
        validate_relative_path(path)?;
        path_bytes += path.as_os_str().as_encoded_bytes().len();
        if path_bytes > MAX_AGENT_GIT_FOCUS_BYTES {
            return Err(format!(
                "agent Git focus exceeds the {MAX_AGENT_GIT_FOCUS_BYTES}-byte path limit"
            ));
        }
        if !unique.insert(path) {
            return Err("agent Git focus contains a duplicate path".into());
        }
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
    if context.calls.len() > MAX_SYMBOL_CALLS {
        return Err(format!(
            "symbol context exceeds the {MAX_SYMBOL_CALLS}-call limit"
        ));
    }
    for call in &context.calls {
        if call.name.is_empty() || call.name.len() > MAX_SYMBOL_NAME_BYTES {
            return Err(format!(
                "call name must contain 1 to {MAX_SYMBOL_NAME_BYTES} bytes"
            ));
        }
        if !(1..=2).contains(&call.depth) {
            return Err("call depth must be one or two".into());
        }
        validate_symbol_location(&call.location)?;
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
            calls: vec![],
        };
        assert_eq!(
            validate_symbol_context(&context).unwrap_err(),
            "symbol context exceeds the 8-reference limit"
        );
    }

    #[test]
    fn rejects_unbounded_or_invalid_symbol_calls() {
        let location = SymbolLocation {
            relative_path: "src/lib.rs".into(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
        };
        let call = SymbolCall {
            name: "dispatch".into(),
            depth: 1,
            location: location.clone(),
        };
        let mut context = SymbolContext {
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
            definition: location,
            references: vec![],
            calls: vec![call; MAX_SYMBOL_CALLS + 1],
        };
        assert_eq!(
            validate_symbol_context(&context).unwrap_err(),
            "symbol context exceeds the 8-call limit"
        );
        context.calls.truncate(1);
        context.calls[0].depth = 3;
        assert_eq!(
            validate_symbol_context(&context).unwrap_err(),
            "call depth must be one or two"
        );
        context.calls[0].depth = 1;
        context.calls[0].name.clear();
        assert_eq!(
            validate_symbol_context(&context).unwrap_err(),
            "call name must contain 1 to 256 bytes"
        );
    }

    #[test]
    fn rejects_unknown_evidence_provenance() {
        let event = r#"{"type":"evidence","id":1,"evidence":{"source":"model_guess","relative_path":"src/lib.rs","start_line":1,"start_column":1,"end_line":1,"end_column":2,"excerpt":"x"}}"#;
        assert!(serde_json::from_str::<Event>(event).is_err());
    }

    #[test]
    fn rejects_unknown_grounding_state() {
        let event = r#"{"type":"grounding_state","id":1,"state":"estimating_progress"}"#;
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
    fn git_review_context_becomes_one_bounded_agent_selection() {
        let review = GitReviewContext {
            relative_path: "src/lib.rs".into(),
            state: GitReviewState::Staged,
            scope: GitReviewScope::Hunk,
            start_line: 4,
            end_line: 7,
            diff: "@@ -4,2 +4,3 @@\n-old\n+new\n".into(),
        };
        validate_git_review(&review).unwrap();
        let selection = review.into_selection();
        validate_selection(&selection).unwrap();
        assert!(selection.text.contains("Git review state: staged"));
        assert!(selection.text.contains("@@ -4,2 +4,3 @@"));
    }

    #[test]
    fn agent_git_focus_is_bounded_unique_and_repository_relative() {
        let focus = AgentGitFocus {
            relative_paths: vec!["src/lib.rs".into(), "tests/flow.rs".into()],
        };
        validate_agent_git_focus(&focus).unwrap();

        let duplicate = AgentGitFocus {
            relative_paths: vec!["src/lib.rs".into(), "src/lib.rs".into()],
        };
        assert_eq!(
            validate_agent_git_focus(&duplicate).unwrap_err(),
            "agent Git focus contains a duplicate path"
        );
        let escape = AgentGitFocus {
            relative_paths: vec!["../outside".into()],
        };
        assert!(validate_agent_git_focus(&escape).is_err());
        let too_many = AgentGitFocus {
            relative_paths: (0..=MAX_AGENT_TOUCHED_PATHS)
                .map(|index| format!("src/{index}.rs").into())
                .collect(),
        };
        assert!(validate_agent_git_focus(&too_many).is_err());
        let oversized = AgentGitFocus {
            relative_paths: vec!["x".repeat(MAX_AGENT_GIT_FOCUS_BYTES + 1).into()],
        };
        assert!(validate_agent_git_focus(&oversized).is_err());
    }

    #[test]
    fn natural_language_intent_defaults_to_read_only_and_respects_negation() {
        for query in [
            "What does this parser do?",
            "Where is add_request implemented?",
            "Explain this selection without changing it",
            "Tell me about the repository",
        ] {
            assert_eq!(
                infer_agent_intent(query, None),
                AgentIntent::Understand,
                "{query}"
            );
        }
        for query in [
            "Look into adding authentication; don't implement it yet",
            "Can we support multiple workbench folders?",
            "Should we add a cache here?",
            "Assess whether this change is safe",
        ] {
            assert_eq!(
                infer_agent_intent(query, None),
                AgentIntent::Investigate,
                "{query}"
            );
        }
        assert_eq!(
            infer_agent_intent("Turn this into a plan", None),
            AgentIntent::Plan
        );
        assert_eq!(
            infer_agent_intent("We should preserve this decision", None),
            AgentIntent::Plan
        );
        for query in ["Write this down", "Save this plan", "Keep this as our plan"] {
            assert_eq!(
                infer_agent_intent(query, Some(AgentIntent::Plan)),
                AgentIntent::PersistPlan,
                "{query}"
            );
        }
        assert_eq!(
            infer_agent_intent(
                "Write this down, but do not implement it yet",
                Some(AgentIntent::Plan)
            ),
            AgentIntent::PersistPlan
        );
        for query in ["Apply that", "Apply the review", "Address all of them"] {
            assert_eq!(
                infer_agent_intent(query, Some(AgentIntent::Plan)),
                AgentIntent::ApplyPlanRevision,
                "{query}"
            );
        }
        for query in [
            "Proceed with the first task",
            "Implement the accepted plan",
            "Fix the failing parser test",
            "Go ahead and apply the change",
        ] {
            assert_eq!(
                infer_agent_intent(query, Some(AgentIntent::Investigate)),
                AgentIntent::Implement,
                "{query}"
            );
        }
    }

    #[test]
    fn natural_language_refinements_continue_the_previous_read_only_work() {
        for query in [
            "Yes, but keep the cache bounded",
            "Only retain entries in memory",
            "Instead use the existing invalidation hook",
        ] {
            assert_eq!(
                infer_agent_intent(query, Some(AgentIntent::Investigate)),
                AgentIntent::Investigate,
                "{query}"
            );
        }
        assert_eq!(
            infer_agent_intent("Also include verification", Some(AgentIntent::Plan)),
            AgentIntent::Plan
        );
        assert_eq!(
            infer_agent_intent("Do it", Some(AgentIntent::Investigate)),
            AgentIntent::Implement
        );
        assert_eq!(
            infer_agent_intent("Yes", Some(AgentIntent::Understand)),
            AgentIntent::Understand
        );
    }

    #[test]
    fn agent_turns_require_one_known_intent() {
        let missing =
            r#"{"method":"ask_agent","id":1,"repository":"/repo","query":"Explain this"}"#;
        assert!(serde_json::from_str::<Request>(missing).is_err());
        let unknown = r#"{"method":"ask_agent","id":1,"repository":"/repo","query":"Explain this","intent":"auto"}"#;
        assert!(serde_json::from_str::<Request>(unknown).is_err());
    }

    #[test]
    fn plan_review_is_bounded_saved_and_anchored_to_the_active_plan() {
        let comment = PlanReviewComment {
            anchor: SelectionContext {
                relative_path: ".lantern/plans/active.md".into(),
                language: Some("markdown".into()),
                start_line: 10,
                start_column: 1,
                end_line: 10,
                end_column: 12,
                text: "One task".into(),
                document_modified: false,
            },
            comment: "Move this after the protocol change".into(),
        };
        assert!(validate_plan_review(std::slice::from_ref(&comment)).is_ok());
        let mut wrong_path = comment.clone();
        wrong_path.anchor.relative_path = "src/lib.rs".into();
        assert!(validate_plan_review(&[wrong_path]).is_err());
        let mut modified = comment.clone();
        modified.anchor.document_modified = true;
        assert!(validate_plan_review(&[modified]).is_err());
        assert!(validate_plan_review(&[]).is_err());
        assert!(validate_plan_review(&vec![comment; MAX_PLAN_REVIEW_COMMENTS + 1]).is_err());
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
