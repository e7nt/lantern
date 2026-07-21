use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};
use lantern_git_rail::{Cancellation, Commit, DiffHunk, GitRail, GitResult, Status, SyncState};
use lantern_protocol::{
    AgentGitFocus, CodeReviewAnchor, CodeReviewComment, ControlRequest, GitReviewContext,
    GitReviewScope, GitReviewState, MAX_AGENT_GIT_FOCUS_BYTES, MAX_CODE_REVIEW_BYTES,
    MAX_CODE_REVIEW_COMMENT_BYTES, MAX_SELECTION_BYTES, validate_agent_git_focus,
    validate_code_review, validate_git_review,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Stdout, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

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
const SELECTED: Color = Color::Rgb {
    r: 71,
    g: 52,
    b: 94,
};
const ACTIONS: [&str; 5] = ["Commit", "Branches", "Fetch", "Pull", "History"];
const HISTORY_LIMIT: usize = 20;
const REFRESH_INTERVAL: Duration = Duration::from_millis(750);
const ASK_AGENT_EXIT_CODE: i32 = 20;
const EXPAND_REVIEW_EXIT_CODE: i32 = 21;
const COLLAPSE_REVIEW_EXIT_CODE: i32 = 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    Compact,
    Review,
}

impl Layout {
    fn from_environment() -> Result<Self, String> {
        match env::var("LANTERN_GIT_LAYOUT").as_deref() {
            Ok("compact") => Ok(Self::Compact),
            Ok("review") => Ok(Self::Review),
            Ok(value) => Err(format!("Unknown Lantern Git layout: {value}")),
            Err(_) => Err("Lantern Git layout is not configured.".into()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GitResume {
    review: GitReviewContext,
    diff_line: usize,
    offset: usize,
    comments: Vec<CodeReviewComment>,
    submitted_comments: Vec<CodeReviewComment>,
}

#[derive(Clone, Copy)]
enum InputKind {
    Commit { staged: usize },
    CreateBranch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChangeKind {
    Conflicted,
    Staged,
    Unstaged,
    Untracked,
}

impl ChangeKind {
    fn label(self) -> &'static str {
        match self {
            Self::Conflicted => "conflict",
            Self::Staged => "staged",
            Self::Unstaged => "modified",
            Self::Untracked => "untracked",
        }
    }

    fn staged(self) -> bool {
        self == Self::Staged
    }

    fn review_state(self) -> GitReviewState {
        match self {
            Self::Conflicted => GitReviewState::Conflict,
            Self::Staged => GitReviewState::Staged,
            Self::Unstaged => GitReviewState::Modified,
            Self::Untracked => GitReviewState::Untracked,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    kind: ChangeKind,
    path: PathBuf,
}

enum View {
    Changes,
    Diff {
        change: Change,
        hunks: Vec<DiffHunk>,
        selected: usize,
        offset: usize,
        cursor: usize,
    },
    Actions {
        selected: usize,
    },
    Input {
        kind: InputKind,
        value: String,
    },
    ReviewInput {
        anchor: CodeReviewAnchor,
        value: String,
        editing: Option<usize>,
    },
    ReviewSummary {
        selected: usize,
    },
    ReviewConfirm,
    Branches {
        branches: Vec<String>,
        selected: usize,
    },
    History {
        commits: Vec<Commit>,
        selected: usize,
    },
    CommitDiff {
        title: String,
        lines: Vec<String>,
        offset: usize,
        history: Vec<Commit>,
        selected: usize,
    },
}

struct State {
    branch: String,
    changes: Vec<Change>,
    selected: usize,
    view: View,
    notice: Option<String>,
    network: Option<NetworkOperation>,
    repository_generation: u64,
    help: bool,
    review_comments: Vec<CodeReviewComment>,
    submitted_review_comments: Vec<CodeReviewComment>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NetworkAction {
    Fetch,
    Pull { commits: usize },
}

impl NetworkAction {
    fn progress(self) -> &'static str {
        match self {
            Self::Fetch => "Fetching…  Esc cancel",
            Self::Pull { .. } => "Pulling…  Esc cancel",
        }
    }
}

struct NetworkOperation {
    action: NetworkAction,
    cancellation: Cancellation,
}

struct NetworkResult {
    action: NetworkAction,
    result: GitResult<()>,
}

#[derive(Clone)]
struct RefreshFocus {
    change: Change,
    hunk_identity: Vec<u8>,
    selected: usize,
    offset: usize,
    cursor: usize,
}

struct RefreshResult {
    generation: u64,
    status: GitResult<Status>,
    focus: Option<RefreshFocus>,
    hunks: Option<GitResult<Vec<DiffHunk>>>,
}

impl State {
    fn load(rail: &GitRail) -> Result<Self, String> {
        let status = rail.status().map_err(|error| error.to_string())?;
        Ok(Self {
            branch: status.branch.clone(),
            changes: changes(status),
            selected: 0,
            view: View::Changes,
            notice: None,
            network: None,
            repository_generation: 0,
            help: false,
            review_comments: Vec::new(),
            submitted_review_comments: Vec::new(),
        })
    }

    fn refresh(&mut self, rail: &GitRail) {
        match rail.status() {
            Ok(status) => {
                self.repository_generation = self.repository_generation.wrapping_add(1);
                self.apply_status(status);
                self.notice = None;
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn apply_status(&mut self, status: Status) {
        let selected = self.changes.get(self.selected).cloned();
        self.branch = status.branch.clone();
        self.changes = changes(status);
        self.selected = selected
            .as_ref()
            .and_then(|selected| self.changes.iter().position(|change| change == selected))
            .or_else(|| {
                selected.as_ref().and_then(|selected| {
                    self.changes
                        .iter()
                        .position(|change| change.path == selected.path)
                })
            })
            .unwrap_or_else(|| self.selected.min(self.changes.len().saturating_sub(1)));
    }

    fn refresh_focus(&self) -> Option<RefreshFocus> {
        let View::Diff {
            change,
            hunks,
            selected,
            offset,
            cursor,
        } = &self.view
        else {
            return None;
        };
        hunks.get(*selected).map(|hunk| RefreshFocus {
            change: change.clone(),
            hunk_identity: hunk.display().to_vec(),
            selected: *selected,
            offset: *offset,
            cursor: *cursor,
        })
    }

    fn apply_background_refresh(&mut self, completed: RefreshResult) {
        if completed.generation != self.repository_generation {
            return;
        }
        let status = match completed.status {
            Ok(status) => status,
            Err(error) => {
                self.notice = Some(error.to_string());
                return;
            }
        };
        self.apply_status(status);
        let Some(focus) = completed.focus else {
            return;
        };
        if self
            .refresh_focus()
            .is_none_or(|current| current.hunk_identity != focus.hunk_identity)
        {
            return;
        }
        let exact = self
            .changes
            .iter()
            .position(|change| change == &focus.change);
        let same_path = self
            .changes
            .iter()
            .position(|change| change.path == focus.change.path);
        let Some(index) = exact.or(same_path) else {
            self.view = View::Changes;
            self.notice = Some(format!("{} is now clean.", focus.change.path.display()));
            return;
        };
        self.selected = index;
        if exact.is_none() {
            self.view = View::Changes;
            self.notice = Some(format!(
                "{} changed Git state; review the refreshed entry.",
                focus.change.path.display()
            ));
            return;
        }
        match completed.hunks {
            Some(Ok(hunks)) if !hunks.is_empty() => {
                let selected = hunks
                    .iter()
                    .position(|hunk| hunk.display() == focus.hunk_identity)
                    .unwrap_or_else(|| focus.selected.min(hunks.len() - 1));
                let line_count = String::from_utf8_lossy(hunks[selected].display())
                    .lines()
                    .count();
                let cursor = reviewable_diff_line(hunks[selected].display());
                self.view = View::Diff {
                    change: focus.change,
                    hunks,
                    selected,
                    offset: focus.offset.min(line_count.saturating_sub(1)),
                    cursor: if focus.cursor < line_count {
                        focus.cursor
                    } else {
                        cursor
                    },
                };
            }
            Some(Err(error)) => {
                self.view = View::Changes;
                self.notice = Some(error.to_string());
            }
            _ => {
                self.view = View::Changes;
                self.notice = Some(format!("{} is now clean.", focus.change.path.display()));
            }
        }
    }

    fn move_selection(&mut self, amount: isize) {
        if self.changes.is_empty() {
            return;
        }
        self.selected = self
            .selected
            .saturating_add_signed(amount)
            .min(self.changes.len() - 1);
    }

    fn move_review_file(&mut self, rail: &GitRail, amount: isize) {
        if self.changes.is_empty() {
            return;
        }
        let mut candidate = self.selected;
        loop {
            let next = candidate
                .saturating_add_signed(amount)
                .min(self.changes.len() - 1);
            if next == candidate {
                return;
            }
            candidate = next;
            if self.changes[candidate].kind != ChangeKind::Conflicted {
                self.selected = candidate;
                self.open_diff(rail);
                return;
            }
        }
    }

    fn toggle_stage(&mut self, rail: &GitRail) {
        let Some(change) = self.changes.get(self.selected) else {
            return;
        };
        if change.kind == ChangeKind::Conflicted {
            self.notice = Some("Resolve this conflict in Helix.".into());
            return;
        }
        let result = if change.kind.staged() {
            rail.unstage(&change.path)
        } else {
            rail.stage(&change.path)
        };
        match result {
            Ok(()) => self.refresh(rail),
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn open_diff(&mut self, rail: &GitRail) {
        let Some(change) = self.changes.get(self.selected) else {
            return;
        };
        match rail.diff_hunks(&change.path, change.kind.staged()) {
            Ok(hunks) => {
                let cursor = hunks
                    .first()
                    .map_or(0, |hunk| reviewable_diff_line(hunk.display()));
                self.view = View::Diff {
                    change: change.clone(),
                    hunks,
                    selected: 0,
                    offset: 0,
                    cursor,
                };
                self.notice = None;
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn move_hunk(&mut self, amount: isize) {
        let View::Diff {
            hunks,
            selected,
            offset,
            cursor,
            ..
        } = &mut self.view
        else {
            return;
        };
        *selected = selected
            .saturating_add_signed(amount)
            .min(hunks.len().saturating_sub(1));
        *offset = 0;
        *cursor = hunks
            .get(*selected)
            .map_or(0, |hunk| reviewable_diff_line(hunk.display()));
    }

    fn move_diff_line(&mut self, amount: isize) {
        let View::Diff {
            hunks,
            selected,
            offset,
            cursor,
            ..
        } = &mut self.view
        else {
            return;
        };
        let line_count = hunks
            .get(*selected)
            .map(|hunk| String::from_utf8_lossy(hunk.display()).lines().count())
            .unwrap_or(0);
        *cursor = cursor
            .saturating_add_signed(amount)
            .min(line_count.saturating_sub(1));
        if *cursor < *offset {
            *offset = *cursor;
        }
    }

    fn toggle_hunk(&mut self, rail: &GitRail) {
        let View::Diff {
            change,
            hunks,
            selected,
            ..
        } = &self.view
        else {
            return;
        };
        let Some(hunk) = hunks.get(*selected) else {
            return;
        };
        let change = change.clone();
        let result = if change.kind.staged() {
            rail.unstage_hunk(hunk.patch())
        } else {
            rail.stage_hunk(hunk.patch())
        };
        let action = if change.kind.staged() {
            "Hunk unstaged."
        } else {
            "Hunk staged."
        };
        match result {
            Ok(()) => {
                self.view = View::Changes;
                self.refresh(rail);
                if let Some(index) = self.changes.iter().position(|candidate| {
                    candidate.kind == change.kind && candidate.path == change.path
                }) {
                    self.selected = index;
                    self.open_diff(rail);
                }
                self.notice = Some(action.into());
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn open_selected_in_helix(&mut self) {
        let (path, start, end) = match &self.view {
            View::Changes => {
                let Some(change) = self.changes.get(self.selected) else {
                    return;
                };
                (change.path.clone(), 1, 2)
            }
            View::Diff {
                change,
                hunks,
                selected,
                ..
            } => {
                let Some(hunk) = hunks.get(*selected) else {
                    return;
                };
                let Some((start, end)) = hunk.navigation_lines() else {
                    self.notice = Some("Deleted hunk has no current Helix range.".into());
                    return;
                };
                (change.path.clone(), start, end)
            }
            _ => return,
        };
        let output = Command::new("lantern-open-range")
            .arg(&path)
            .args([start.to_string(), "1".into(), end.to_string(), "1".into()])
            .output();
        match output {
            Ok(output) if output.status.success() => {
                self.notice = Some(format!("Opened {} in Helix.", path.display()));
            }
            Ok(output) => {
                let detail = String::from_utf8_lossy(&output.stderr);
                self.notice = Some(if detail.trim().is_empty() {
                    "Helix navigation failed without diagnostics.".into()
                } else {
                    format!("Helix navigation failed: {}", detail.trim())
                });
            }
            Err(cause) => self.notice = Some(format!("Cannot open Helix range: {cause}")),
        }
    }

    fn open_actions(&mut self) {
        self.view = View::Actions { selected: 0 };
        self.notice = None;
    }

    fn move_menu(&mut self, amount: isize) {
        let (selected, length) = match &mut self.view {
            View::Actions { selected } => (selected, ACTIONS.len()),
            View::Branches { branches, selected } => (selected, branches.len() + 1),
            View::History { commits, selected } => (selected, commits.len()),
            _ => return,
        };
        if length == 0 {
            return;
        }
        *selected = selected
            .saturating_add_signed(amount)
            .min(length.saturating_sub(1));
    }

    fn choose_action(&mut self, rail: &GitRail, sender: &Sender<NetworkResult>) {
        let View::Actions { selected } = self.view else {
            return;
        };
        match selected {
            0 => match rail.status() {
                Ok(status) if status.staged.is_empty() => {
                    self.view = View::Changes;
                    self.notice = Some("Stage at least one change before committing.".into());
                }
                Ok(status) => {
                    self.view = View::Input {
                        kind: InputKind::Commit {
                            staged: status.staged.len(),
                        },
                        value: String::new(),
                    };
                }
                Err(message) => self.fail_action(message.to_string()),
            },
            1 => match rail.local_branches() {
                Ok(branches) => {
                    self.view = View::Branches {
                        branches,
                        selected: 0,
                    };
                }
                Err(message) => self.fail_action(message.to_string()),
            },
            2 => self.start_network(rail, sender, NetworkAction::Fetch),
            3 => self.pull(rail, sender),
            4 => match rail.recent_commits(HISTORY_LIMIT) {
                Ok(commits) => {
                    self.view = View::History {
                        commits,
                        selected: 0,
                    };
                }
                Err(message) => self.fail_action(message.to_string()),
            },
            _ => {}
        }
    }

    fn pull(&mut self, rail: &GitRail, sender: &Sender<NetworkResult>) {
        let state = match rail.sync_state() {
            Ok(state) => state,
            Err(message) => {
                self.fail_action(message.to_string());
                return;
            }
        };
        let notice = match state {
            SyncState::Behind { commits } => {
                self.start_network(rail, sender, NetworkAction::Pull { commits });
                return;
            }
            SyncState::UpToDate => "Already up to date.".into(),
            SyncState::Ahead { commits } => format!("Ahead by {commits}; nothing to pull."),
            SyncState::Diverged { ahead, behind } => {
                format!("Diverged: {ahead} ahead, {behind} behind. Resolve explicitly.")
            }
            SyncState::NoUpstream => "No upstream branch is configured.".into(),
        };
        self.view = View::Changes;
        self.refresh(rail);
        self.notice = Some(notice);
    }

    fn start_network(
        &mut self,
        rail: &GitRail,
        sender: &Sender<NetworkResult>,
        action: NetworkAction,
    ) {
        if self.network.is_some() {
            return;
        }
        let cancellation = Cancellation::default();
        let worker_cancellation = cancellation.clone();
        let worker_rail = rail.clone();
        let sender = sender.clone();
        thread::spawn(move || {
            let result = match action {
                NetworkAction::Fetch => worker_rail.fetch_with_cancellation(worker_cancellation),
                NetworkAction::Pull { .. } => {
                    worker_rail.pull_fast_forward_with_cancellation(worker_cancellation)
                }
            };
            let _ = sender.send(NetworkResult { action, result });
        });
        self.view = View::Changes;
        self.notice = Some(action.progress().into());
        self.network = Some(NetworkOperation {
            action,
            cancellation,
        });
    }

    fn finish_network(&mut self, rail: &GitRail, completed: NetworkResult) {
        let Some(active) = self.network.take() else {
            return;
        };
        if active.action != completed.action {
            return;
        }
        match completed.result {
            Ok(()) => {
                self.refresh(rail);
                self.notice = Some(match completed.action {
                    NetworkAction::Fetch => "Fetched remote state.".into(),
                    NetworkAction::Pull { commits } => {
                        format!("Fast-forwarded {commits} commit(s).")
                    }
                });
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn cancel_network(&mut self) {
        if let Some(operation) = &self.network {
            operation.cancellation.cancel();
            self.notice = Some("Cancelling Git operation…".into());
        }
    }

    fn review_context(&self, rail: &GitRail) -> Result<GitReviewContext, String> {
        let scope = if matches!(&self.view, View::Diff { .. }) {
            GitReviewScope::Hunk
        } else {
            GitReviewScope::File
        };
        let (change, hunks) = match &self.view {
            View::Changes => {
                let change = self
                    .changes
                    .get(self.selected)
                    .cloned()
                    .ok_or("Select a Git change before asking Lantern.")?;
                let hunks = if change.kind == ChangeKind::Conflicted {
                    Vec::new()
                } else {
                    rail.diff_hunks(&change.path, change.kind.staged())
                        .map_err(|error| error.to_string())?
                };
                (change, hunks)
            }
            View::Diff {
                change,
                hunks,
                selected,
                ..
            } => {
                let hunk = hunks
                    .get(*selected)
                    .cloned()
                    .ok_or("Select a Git hunk before asking Lantern.")?;
                (change.clone(), vec![hunk])
            }
            _ => return Err("Open a changed file or hunk before asking Lantern.".into()),
        };
        let (start_line, end_line, diff) = if hunks.is_empty() {
            (
                1,
                2,
                "This file is conflicted. Inspect the working tree and Git stages before answering."
                    .into(),
            )
        } else {
            let mut start_line = usize::MAX;
            let mut end_line = 0;
            let mut diff = String::new();
            for hunk in hunks {
                if let Some((start, end)) = hunk.navigation_lines() {
                    start_line = start_line.min(start);
                    end_line = end_line.max(end);
                }
                if !diff.is_empty() {
                    diff.push('\n');
                }
                diff.push_str(&String::from_utf8_lossy(hunk.display()));
            }
            if start_line == usize::MAX {
                start_line = 1;
                end_line = 2;
            }
            (start_line, end_line.max(start_line + 1), diff)
        };
        let context = GitReviewContext {
            relative_path: change.path,
            state: change.kind.review_state(),
            scope,
            start_line,
            end_line,
            diff,
        };
        validate_git_review(&context).map_err(|message| {
            if message.contains("exceeds") && matches!(&self.view, View::Changes) {
                "This file review is too large. Open the exact hunk, then press Ctrl-a.".into()
            } else {
                message
            }
        })?;
        Ok(context)
    }

    fn export_review_context(&self, rail: &GitRail) -> Result<(), String> {
        let context = self.review_context(rail)?;
        let destination = env::var_os("LANTERN_REVIEW_PATH")
            .map(PathBuf::from)
            .ok_or("Lantern review context path is not configured.")?;
        let resume = env::var_os("LANTERN_GIT_RESUME_PATH")
            .map(PathBuf::from)
            .ok_or("Lantern Git resume path is not configured.")?;
        let payload = serde_json::to_vec(&context)
            .map_err(|error| format!("Cannot encode Git review context: {error}"))?;
        self.publish_resume_context(rail, &resume)?;
        if let Err(message) = publish_context(&destination, &payload) {
            let _ = fs::remove_file(resume);
            return Err(message);
        }
        Ok(())
    }

    fn publish_resume_context(&self, rail: &GitRail, destination: &Path) -> Result<(), String> {
        let review = self.review_context(rail)?;
        let (diff_line, offset) = match &self.view {
            View::Diff { cursor, offset, .. } => (*cursor, *offset),
            _ => (0, 0),
        };
        let resume = GitResume {
            review,
            diff_line,
            offset,
            comments: self.review_comments.clone(),
            submitted_comments: self.submitted_review_comments.clone(),
        };
        let payload = serde_json::to_vec(&resume)
            .map_err(|error| format!("Cannot encode Git review position: {error}"))?;
        publish_context(destination, &payload)
    }

    fn code_review_anchor(&self, rail: &GitRail) -> Result<CodeReviewAnchor, String> {
        let review = self.review_context(rail)?;
        let View::Diff { cursor, .. } = &self.view else {
            return Err("Open a diff hunk before adding a review comment.".into());
        };
        let line = review
            .diff
            .lines()
            .nth(*cursor)
            .ok_or("Select a line inside the current diff hunk.")?
            .to_owned();
        let anchor = CodeReviewAnchor {
            review,
            diff_line: *cursor,
            line,
        };
        validate_code_review(&[CodeReviewComment {
            anchor: anchor.clone(),
            comment: "validate".into(),
        }])?;
        Ok(anchor)
    }

    fn start_code_review_comment(&mut self, rail: &GitRail) {
        match self.code_review_anchor(rail) {
            Ok(anchor) => {
                let editing = self
                    .review_comments
                    .iter()
                    .position(|comment| comment.anchor == anchor);
                let value = editing
                    .map(|index| self.review_comments[index].comment.clone())
                    .unwrap_or_default();
                self.view = View::ReviewInput {
                    anchor,
                    value,
                    editing,
                };
                self.notice = None;
            }
            Err(message) => self.notice = Some(message),
        }
    }

    fn submit_code_review_comment(&mut self, rail: &GitRail) {
        let View::ReviewInput {
            anchor,
            value,
            editing,
        } = &self.view
        else {
            return;
        };
        let review = anchor.review.clone();
        let diff_line = anchor.diff_line;
        let comment = CodeReviewComment {
            anchor: anchor.clone(),
            comment: value.clone(),
        };
        if let Err(message) = validate_code_review(std::slice::from_ref(&comment)) {
            self.notice = Some(message);
            return;
        }
        let mut comments = self.review_comments.clone();
        if let Some(index) = editing {
            comments[*index] = comment;
        } else {
            comments.push(comment);
        }
        if let Err(message) = validate_code_review(&comments) {
            self.notice = Some(message);
            return;
        }
        self.review_comments = comments;
        self.restore_review(rail, &review);
        if let View::Diff { cursor, offset, .. } = &mut self.view {
            *cursor = diff_line;
            *offset = diff_line.saturating_sub(1);
        }
        self.notice = Some(format!(
            "Review draft has {} comment{}. Press v to inspect or R to submit.",
            self.review_comments.len(),
            if self.review_comments.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }

    fn submit_code_review(&mut self) -> Result<(), String> {
        validate_code_review(&self.review_comments)?;
        send_control_request(&ControlRequest::SubmitCodeReview {
            comments: self.review_comments.clone(),
        })
    }

    fn comment_at_cursor(&self, rail: &GitRail) -> Option<usize> {
        let anchor = self.code_review_anchor(rail).ok()?;
        self.review_comments
            .iter()
            .position(|comment| comment.anchor == anchor)
    }

    fn edit_comment_at_cursor(&mut self, rail: &GitRail) {
        let Some(index) = self.comment_at_cursor(rail) else {
            self.notice = Some("This line has no review comment.".into());
            return;
        };
        let comment = self.review_comments[index].clone();
        self.view = View::ReviewInput {
            anchor: comment.anchor,
            value: comment.comment,
            editing: Some(index),
        };
    }

    fn delete_comment_at_cursor(&mut self, rail: &GitRail) {
        let Some(index) = self.comment_at_cursor(rail) else {
            self.notice = Some("This line has no review comment.".into());
            return;
        };
        self.review_comments.remove(index);
        self.notice = Some("Removed the review comment from this line.".into());
    }

    fn open_review_comment(&mut self, rail: &GitRail, index: usize) {
        let Some(comment) = self.review_summary_comment(index).cloned() else {
            return;
        };
        self.restore_review(rail, &comment.anchor.review);
        if let View::Diff { cursor, offset, .. } = &mut self.view {
            *cursor = comment.anchor.diff_line;
            *offset = comment.anchor.diff_line.saturating_sub(1);
        }
    }

    fn review_summary_comment(&self, index: usize) -> Option<&CodeReviewComment> {
        self.submitted_review_comments.get(index).or_else(|| {
            index
                .checked_sub(self.submitted_review_comments.len())
                .and_then(|index| self.review_comments.get(index))
        })
    }

    fn review_summary_len(&self) -> usize {
        self.submitted_review_comments.len() + self.review_comments.len()
    }

    fn restore_review_context(&mut self, rail: &GitRail, path: &Path, open_diff: bool) {
        let Ok(bytes) = fs::read(path) else {
            return;
        };
        let _ = fs::remove_file(path);
        if bytes.len() > MAX_SELECTION_BYTES + 2 * MAX_CODE_REVIEW_BYTES + 8 * 1024 {
            self.notice = Some("Saved Git review context exceeded its bound.".into());
            return;
        }
        let Ok(resume) = serde_json::from_slice::<GitResume>(&bytes) else {
            self.notice = Some("Saved Git review context was invalid.".into());
            return;
        };
        if let Err(message) = validate_git_review(&resume.review) {
            self.notice = Some(message);
            return;
        }
        if !resume.comments.is_empty() && validate_code_review(&resume.comments).is_err() {
            self.notice = Some("Saved Git review draft was invalid.".into());
            return;
        }
        if !resume.submitted_comments.is_empty()
            && validate_code_review(&resume.submitted_comments).is_err()
        {
            self.notice = Some("Saved submitted review was invalid.".into());
            return;
        }
        self.restore_review(rail, &resume.review);
        self.review_comments = resume.comments;
        self.submitted_review_comments = resume.submitted_comments;
        if open_diff {
            if let View::Diff {
                hunks,
                selected,
                cursor,
                offset,
                ..
            } = &mut self.view
            {
                let line_count = hunks
                    .get(*selected)
                    .map(|hunk| String::from_utf8_lossy(hunk.display()).lines().count())
                    .unwrap_or(0);
                *cursor = resume.diff_line.min(line_count.saturating_sub(1));
                *offset = resume.offset.min(line_count.saturating_sub(1));
            }
        } else {
            self.view = View::Changes;
        }
    }

    fn restore_review(&mut self, rail: &GitRail, context: &GitReviewContext) {
        let selected = self
            .changes
            .iter()
            .position(|change| {
                change.path == context.relative_path && change.kind.review_state() == context.state
            })
            .or_else(|| {
                self.changes
                    .iter()
                    .position(|change| change.path == context.relative_path)
            });
        let Some(selected) = selected else {
            self.notice = Some(format!("{} is now clean.", context.relative_path.display()));
            return;
        };
        self.selected = selected;
        if context.scope != GitReviewScope::Hunk
            || self.changes[selected].kind == ChangeKind::Conflicted
        {
            return;
        }
        let change = self.changes[selected].clone();
        match rail.diff_hunks(&change.path, change.kind.staged()) {
            Ok(hunks) if !hunks.is_empty() => {
                let selected = hunks
                    .iter()
                    .position(|hunk| hunk.display() == context.diff.as_bytes())
                    .or_else(|| {
                        hunks
                            .iter()
                            .enumerate()
                            .filter_map(|(index, hunk)| {
                                hunk.navigation_lines()
                                    .map(|(start, _)| (index, start.abs_diff(context.start_line)))
                            })
                            .min_by_key(|(_, distance)| *distance)
                            .map(|(index, _)| index)
                    })
                    .unwrap_or(0);
                let cursor = reviewable_diff_line(hunks[selected].display());
                self.view = View::Diff {
                    change,
                    hunks,
                    selected,
                    offset: 0,
                    cursor,
                };
            }
            Ok(_) => self.notice = Some(format!("{} is now clean.", change.path.display())),
            Err(error) => self.notice = Some(error.to_string()),
        }
    }

    fn focus_agent_changes(&mut self, path: &Path) {
        if fs::metadata(path).is_ok_and(|metadata| {
            metadata.len() > (MAX_AGENT_GIT_FOCUS_BYTES + MAX_CODE_REVIEW_BYTES + 8 * 1024) as u64
        }) {
            let _ = fs::remove_file(path);
            self.notice = Some("Agent Git review context exceeded its bound.".into());
            return;
        }
        let Ok(bytes) = fs::read(path) else {
            return;
        };
        let _ = fs::remove_file(path);
        let Ok(focus) = serde_json::from_slice::<AgentGitFocus>(&bytes) else {
            self.notice = Some("Agent Git review context was invalid.".into());
            return;
        };
        if let Err(message) = validate_agent_git_focus(&focus) {
            self.notice = Some(message);
            return;
        }
        self.submitted_review_comments = focus.review_comments;
        let selected = focus
            .relative_paths
            .iter()
            .find_map(|path| self.changes.iter().position(|change| change.path == *path));
        let Some(selected) = selected else {
            self.notice = Some("The files edited by the agent are now clean.".into());
            return;
        };
        let already_reviewing = match &self.view {
            View::Diff { change, .. } => focus.relative_paths.contains(&change.path),
            _ => false,
        };
        if !already_reviewing {
            self.view = View::Changes;
            self.selected = selected;
        }
        let changed = focus
            .relative_paths
            .iter()
            .filter(|path| self.changes.iter().any(|change| change.path == **path))
            .count();
        let submitted = self.submitted_review_comments.len();
        self.notice = Some(format!(
            "Reviewing {changed} agent-edited file{}{}.",
            if changed == 1 { "" } else { "s" },
            if submitted == 0 {
                String::new()
            } else {
                format!(
                    " · Your review: {submitted} comment{}",
                    if submitted == 1 { "" } else { "s" }
                )
            }
        ));
    }

    fn fail_action(&mut self, message: String) {
        self.view = View::Changes;
        self.notice = Some(message);
    }

    fn edit_input(&mut self, code: KeyCode) {
        let (value, limit) = match &mut self.view {
            View::Input { kind, value } => {
                let limit = match kind {
                    InputKind::Commit { .. } => 4_096,
                    InputKind::CreateBranch => 255,
                };
                (value, limit)
            }
            View::ReviewInput { value, .. } => (value, MAX_CODE_REVIEW_COMMENT_BYTES),
            _ => return,
        };
        match code {
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Char(character) if value.len() + character.len_utf8() <= limit => {
                value.push(character);
            }
            _ => {}
        }
        self.notice = None;
    }

    fn submit_input(&mut self, rail: &GitRail) {
        let View::Input { kind, value } = &self.view else {
            return;
        };
        let kind = *kind;
        let value = value.clone();
        let result = match kind {
            InputKind::Commit { .. } => rail.commit(&value),
            InputKind::CreateBranch => rail.create_branch(&value),
        };
        match result {
            Ok(()) => {
                self.view = View::Changes;
                self.refresh(rail);
                self.notice = Some(match kind {
                    InputKind::Commit { .. } => "Committed staged changes.".into(),
                    InputKind::CreateBranch => format!("Created and switched to {value}."),
                });
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn choose_branch(&mut self, rail: &GitRail) {
        let View::Branches { branches, selected } = &self.view else {
            return;
        };
        if *selected == 0 {
            self.view = View::Input {
                kind: InputKind::CreateBranch,
                value: String::new(),
            };
            return;
        }
        let branch = branches[*selected - 1].clone();
        match rail.switch_branch(&branch) {
            Ok(()) => {
                self.view = View::Changes;
                self.refresh(rail);
                self.notice = Some(format!("Switched to {branch}."));
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn open_history_diff(&mut self, rail: &GitRail) {
        let View::History { commits, selected } = &self.view else {
            return;
        };
        let Some(commit) = commits.get(*selected) else {
            return;
        };
        let title = format!("{} {}", &commit.id[..8], commit.summary);
        let history = commits.clone();
        let selected = *selected;
        match rail.commit_diff(&commit.id) {
            Ok(diff) => {
                self.view = View::CommitDiff {
                    title,
                    lines: String::from_utf8_lossy(&diff)
                        .lines()
                        .map(ToOwned::to_owned)
                        .collect(),
                    offset: 0,
                    history,
                    selected,
                };
            }
            Err(message) => self.notice = Some(message.to_string()),
        }
    }

    fn click_menu(
        &mut self,
        row: usize,
        available: usize,
        rail: &GitRail,
        sender: &Sender<NetworkResult>,
    ) {
        match &self.view {
            View::Actions { selected } => {
                let index = visible_start(*selected, available) + row;
                if index < ACTIONS.len() {
                    self.view = View::Actions { selected: index };
                    self.choose_action(rail, sender);
                }
            }
            View::Branches { branches, selected } => {
                let index = visible_start(*selected, available) + row;
                if index < branches.len() + 1 {
                    if let View::Branches { selected, .. } = &mut self.view {
                        *selected = index;
                    }
                    self.choose_branch(rail);
                }
            }
            View::History { commits, selected } => {
                let index = visible_start(*selected, available) + row;
                if index < commits.len() {
                    if let View::History { selected, .. } = &mut self.view {
                        *selected = index;
                    }
                    self.open_history_diff(rail);
                }
            }
            _ => {}
        }
    }
}

fn changes(status: Status) -> Vec<Change> {
    let mut changes = Vec::new();
    for (kind, paths) in [
        (ChangeKind::Conflicted, status.conflicted),
        (ChangeKind::Staged, status.staged),
        (ChangeKind::Unstaged, status.unstaged),
        (ChangeKind::Untracked, status.untracked),
    ] {
        for path in paths {
            if kind != ChangeKind::Conflicted
                && changes.iter().any(|change: &Change| {
                    change.kind == ChangeKind::Conflicted && change.path == path
                })
            {
                continue;
            }
            changes.push(Change { kind, path });
        }
    }
    changes
}

fn publish_context(destination: &Path, payload: &[u8]) -> Result<(), String> {
    let temporary = destination.with_extension("tmp");
    fs::write(&temporary, payload)
        .map_err(|error| format!("Cannot write Git review context: {error}"))?;
    fs::rename(&temporary, destination)
        .map_err(|error| format!("Cannot publish Git review context: {error}"))
}

#[cfg(unix)]
fn send_control_request(request: &ControlRequest) -> Result<(), String> {
    let socket = env::var_os("LANTERN_CONTROL_SOCKET")
        .map(PathBuf::from)
        .ok_or("Lantern control socket is not configured.")?;
    let mut stream = UnixStream::connect(socket)
        .map_err(|cause| format!("Cannot reach the Lantern agent: {cause}"))?;
    serde_json::to_writer(&mut stream, request)
        .map_err(|cause| format!("Cannot encode the review request: {cause}"))?;
    stream
        .write_all(b"\n")
        .map_err(|cause| format!("Cannot frame the review request: {cause}"))?;
    stream
        .flush()
        .map_err(|cause| format!("Cannot submit the review request: {cause}"))
}

fn clipped(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_owned();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    value.chars().take(width - 1).chain(['…']).collect()
}

fn clipped_path(path: &Path, width: usize) -> String {
    let value = path.to_string_lossy();
    if value.chars().count() <= width {
        return value.into_owned();
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or(value);
    if width <= 2 {
        return clipped(&name, width);
    }
    let available = width - 2;
    let name = if name.chars().count() > available {
        clipped(&name, available)
    } else {
        name.into_owned()
    };
    format!("…/{name}")
}

fn change_row(change: &Change, width: usize, selected: bool) -> String {
    let focus = if selected { "> " } else { "  " };
    let label = change.kind.label();
    let path_width = width.saturating_sub(focus.len() + label.chars().count() + 1);
    clipped(
        &format!("{focus}{label} {}", clipped_path(&change.path, path_width)),
        width,
    )
}

fn visible_start(selected: usize, available: usize) -> usize {
    selected.saturating_sub(available.saturating_sub(1))
}

fn request_background_refresh(
    rail: &GitRail,
    generation: u64,
    focus: Option<RefreshFocus>,
    sender: &Sender<RefreshResult>,
) {
    let rail = rail.clone();
    let sender = sender.clone();
    thread::spawn(move || {
        let status = rail.status();
        let hunks = if status.is_ok() {
            focus
                .as_ref()
                .map(|focus| rail.diff_hunks(&focus.change.path, focus.change.kind.staged()))
        } else {
            None
        };
        let _ = sender.send(RefreshResult {
            generation,
            status,
            focus,
            hunks,
        });
    });
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(cause) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = disable_raw_mode();
            return Err(cause);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            Show,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}

fn draw(stdout: &mut Stdout, state: &State) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    queue!(
        stdout,
        SetBackgroundColor(CANVAS),
        SetForegroundColor(TEXT),
        Clear(ClearType::All),
        MoveTo(0, 0),
        Print(clipped(&format!("Git  {}", state.branch), width as usize))
    )?;
    match &state.view {
        View::Changes => draw_changes(stdout, state, width, height)?,
        View::Diff {
            change,
            hunks,
            selected,
            offset,
            cursor,
        } => draw_diff(
            stdout,
            change,
            hunks,
            (
                *selected,
                *offset,
                *cursor,
                state.submitted_review_comments.len(),
            ),
            &state.review_comments,
            width,
            height,
        )?,
        View::Actions { selected } => {
            draw_menu(stdout, "Actions", &ACTIONS, *selected, width, height)?
        }
        View::Input { kind, value } => draw_input(stdout, *kind, value, width, height)?,
        View::ReviewInput { anchor, value, .. } => {
            draw_review_input(stdout, anchor, value, width, height)?
        }
        View::ReviewSummary { selected } => {
            let mut rows = state
                .submitted_review_comments
                .iter()
                .map(|comment| {
                    format!(
                        "Your review · {}:{}  {}",
                        comment.anchor.review.relative_path.display(),
                        comment.anchor.diff_line + 1,
                        comment.comment
                    )
                })
                .collect::<Vec<_>>();
            rows.extend(state.review_comments.iter().map(|comment| {
                format!(
                    "Draft · {}:{}  {}",
                    comment.anchor.review.relative_path.display(),
                    comment.anchor.diff_line + 1,
                    comment.comment
                )
            }));
            draw_menu(stdout, "Review", &rows, *selected, width, height)?;
        }
        View::ReviewConfirm => {
            draw_review_confirm(stdout, state.review_comments.len(), width, height)?
        }
        View::Branches { branches, selected } => {
            let mut rows = vec!["+ Create branch".to_owned()];
            rows.extend(branches.iter().map(|branch| {
                if branch == &state.branch {
                    format!("• {branch}")
                } else {
                    format!("  {branch}")
                }
            }));
            draw_menu(stdout, "Branches", &rows, *selected, width, height)?;
        }
        View::History { commits, selected } => {
            let rows = commits
                .iter()
                .map(|commit| format!("{} {}", &commit.id[..8], commit.summary))
                .collect::<Vec<_>>();
            draw_menu(stdout, "History", &rows, *selected, width, height)?;
        }
        View::CommitDiff {
            title,
            lines,
            offset,
            ..
        } => draw_commit_diff(stdout, title, lines, *offset, width, height)?,
    }
    if state.help {
        draw_help(stdout, &state.view, width, height)?;
    } else if let Some(notice) = &state.notice {
        queue!(
            stdout,
            MoveTo(0, height.saturating_sub(1)),
            SetBackgroundColor(CANVAS),
            SetForegroundColor(Color::Rgb {
                r: 214,
                g: 120,
                b: 181,
            }),
            Print(clipped(notice, width as usize))
        )?;
    }
    stdout.flush()
}

fn draw_help(stdout: &mut Stdout, view: &View, width: u16, height: u16) -> io::Result<()> {
    let (title, rows): (&str, &[&str]) = match view {
        View::Changes => (
            "Changes help",
            &[
                "↑/k ↓/j  select",
                "↵/d  review diff",
                "space  stage / unstage",
                "o  open in Helix",
                "Ctrl-a  ask Lantern",
                "a  Git actions",
                "r  refresh",
                "mouse: left review",
                "mouse: right stage",
                "mouse: middle open",
                "q/Esc  quit",
            ],
        ),
        View::Diff { .. } => (
            "Diff help",
            &[
                "↑/k ↓/j  select line",
                "p/n  previous/next file",
                "[/]  previous/next hunk",
                "c/right click  comment",
                "e/x  edit/remove comment",
                "v  inspect review draft",
                "R  submit review",
                "space  stage / unstage",
                "↵/o  open in Helix",
                "Ctrl-a  ask Lantern",
                "mouse wheel  scroll",
                "mouse: middle open",
                "Esc  collapse review",
            ],
        ),
        View::Actions { .. } => (
            "Actions help",
            &["↑/k ↓/j  select", "↵/left click  choose", "Esc  changes"],
        ),
        View::Branches { .. } => (
            "Branches help",
            &["↑/k ↓/j  select", "↵/left click  choose", "Esc  actions"],
        ),
        View::History { .. } => (
            "History help",
            &["↑/k ↓/j  select", "↵/left click  inspect", "Esc  actions"],
        ),
        View::CommitDiff { .. } => (
            "History diff help",
            &["↑/k ↓/j  scroll", "PgUp/PgDn  scroll", "Esc  history"],
        ),
        View::Input { .. } => return Ok(()),
        View::ReviewInput { .. } => return Ok(()),
        View::ReviewSummary { .. } | View::ReviewConfirm => return Ok(()),
    };
    queue!(
        stdout,
        MoveTo(0, 1),
        SetBackgroundColor(CANVAS),
        Clear(ClearType::FromCursorDown),
        SetForegroundColor(ACCENT),
        Print(clipped(title, width as usize))
    )?;
    for (row, item) in rows
        .iter()
        .take(height.saturating_sub(4) as usize)
        .enumerate()
    {
        queue!(
            stdout,
            MoveTo(0, row as u16 + 2),
            SetForegroundColor(TEXT),
            Print(clipped(item, width as usize))
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("? / Esc close", width as usize))
    )
}

fn draw_menu(
    stdout: &mut Stdout,
    title: &str,
    rows: &[impl AsRef<str>],
    selected: usize,
    width: u16,
    height: u16,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(title, width as usize))
    )?;
    let available = height.saturating_sub(4) as usize;
    let start = visible_start(selected, available);
    for (row, (index, item)) in rows
        .iter()
        .enumerate()
        .skip(start)
        .take(available)
        .enumerate()
    {
        let selected = index == selected;
        queue!(
            stdout,
            MoveTo(0, row as u16 + 2),
            SetBackgroundColor(if selected { SELECTED } else { CANVAS }),
            SetForegroundColor(if selected { ACCENT } else { TEXT }),
            Print(clipped(
                &format!("{}{}", if selected { "> " } else { "  " }, item.as_ref()),
                width as usize,
            )),
            ResetColor,
            SetBackgroundColor(CANVAS)
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("↵ choose  Esc back", width as usize))
    )
}

fn draw_input(
    stdout: &mut Stdout,
    kind: InputKind,
    value: &str,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let title = match kind {
        InputKind::Commit { staged } => format!("Commit {staged} staged"),
        InputKind::CreateBranch => "New branch".into(),
    };
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(&title, width as usize)),
        MoveTo(0, 3),
        SetForegroundColor(TEXT),
        Print(clipped(&format!("> {value}│"), width as usize)),
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("↵ confirm  Esc cancel", width as usize))
    )
}

fn draw_review_input(
    stdout: &mut Stdout,
    anchor: &CodeReviewAnchor,
    value: &str,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let input_width = width.saturating_sub(2) as usize;
    let mut chunks = value
        .chars()
        .collect::<Vec<_>>()
        .chunks(input_width.max(1))
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>();
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    let available = height.saturating_sub(6) as usize;
    let start = chunks.len().saturating_sub(available.max(1));
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(
            &format!("Comment · {}", anchor.review.relative_path.display()),
            width as usize,
        )),
        MoveTo(0, 2),
        SetForegroundColor(ACCENT),
        Print(clipped(&anchor.line, width as usize)),
    )?;
    for (row, chunk) in chunks.iter().skip(start).take(available.max(1)).enumerate() {
        let cursor = if start + row + 1 == chunks.len() {
            "│"
        } else {
            ""
        };
        queue!(
            stdout,
            MoveTo(0, row as u16 + 4),
            SetForegroundColor(TEXT),
            Print(clipped(
                &format!(
                    "{}{}{}",
                    if start + row == 0 { "> " } else { "  " },
                    chunk,
                    cursor
                ),
                width as usize
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("↵ add  Esc cancel", width as usize))
    )
}

fn draw_review_confirm(
    stdout: &mut Stdout,
    count: usize,
    width: u16,
    height: u16,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, 2),
        SetForegroundColor(TEXT),
        Print(clipped(
            &format!(
                "Submit {count} review comment{} as one correction turn?",
                if count == 1 { "" } else { "s" }
            ),
            width as usize,
        )),
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("↵/y submit  Esc keep editing", width as usize))
    )
}

fn draw_commit_diff(
    stdout: &mut Stdout,
    title: &str,
    lines: &[String],
    offset: usize,
    width: u16,
    height: u16,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(title, width as usize))
    )?;
    for (row, line) in lines
        .iter()
        .skip(offset)
        .take(height.saturating_sub(4) as usize)
        .enumerate()
    {
        let color = if line.starts_with('+') && !line.starts_with("+++") {
            ACCENT
        } else if line.starts_with('-') && !line.starts_with("---") {
            Color::Rgb {
                r: 214,
                g: 120,
                b: 181,
            }
        } else {
            TEXT
        };
        queue!(
            stdout,
            MoveTo(0, row as u16 + 2),
            SetForegroundColor(color),
            Print(clipped(line, width as usize))
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("j/k scroll  Esc back", width as usize))
    )
}

fn draw_changes(stdout: &mut Stdout, state: &State, width: u16, height: u16) -> io::Result<()> {
    let available = height.saturating_sub(3) as usize;
    let start = visible_start(state.selected, available);
    if state.changes.is_empty() {
        queue!(
            stdout,
            MoveTo(0, 2),
            SetForegroundColor(MUTED),
            Print("Clean")
        )?;
    }
    for (row, (index, change)) in state
        .changes
        .iter()
        .enumerate()
        .skip(start)
        .take(available)
        .enumerate()
    {
        let selected = index == state.selected;
        queue!(
            stdout,
            MoveTo(0, row as u16 + 1),
            SetBackgroundColor(if selected { SELECTED } else { CANVAS }),
            SetForegroundColor(if selected { ACCENT } else { TEXT }),
            Print(change_row(change, width as usize, selected)),
            ResetColor,
            SetBackgroundColor(CANVAS)
        )?;
    }
    let footer = state
        .notice
        .as_deref()
        .unwrap_or("? help  a actions  ↵ diff  space stage");
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped(footer, width as usize))
    )
}

fn draw_diff(
    stdout: &mut Stdout,
    change: &Change,
    hunks: &[DiffHunk],
    position: (usize, usize, usize, usize),
    comments: &[CodeReviewComment],
    width: u16,
    height: u16,
) -> io::Result<()> {
    let (selected, offset, cursor, submitted_comments) = position;
    let Some(hunk) = hunks.get(selected) else {
        return Ok(());
    };
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(
            &format!(
                "{}/{} {}{}",
                selected + 1,
                hunks.len(),
                change.path.display(),
                if submitted_comments == 0 {
                    String::new()
                } else {
                    format!(
                        "  Your review · {submitted_comments} comment{} · v view",
                        if submitted_comments == 1 { "" } else { "s" }
                    )
                }
            ),
            width as usize,
        ))
    )?;
    let display = String::from_utf8_lossy(hunk.display());
    let available = height.saturating_sub(3) as usize;
    let start = diff_visible_start(cursor, offset, available);
    for (row, (index, line)) in display
        .lines()
        .enumerate()
        .skip(start)
        .take(available)
        .enumerate()
    {
        let selected = index == cursor;
        let comment_count = comments_at_line(comments, change, hunk, index);
        let color = if line.starts_with('+') && !line.starts_with("+++") {
            ACCENT
        } else if line.starts_with('-') && !line.starts_with("---") {
            Color::Rgb {
                r: 214,
                g: 120,
                b: 181,
            }
        } else {
            TEXT
        };
        queue!(
            stdout,
            MoveTo(0, row as u16 + 2),
            SetBackgroundColor(if selected { SELECTED } else { CANVAS }),
            SetForegroundColor(color),
            Print(clipped(
                &format!("{}{}", if comment_count > 0 { "● " } else { "  " }, line),
                width as usize,
            )),
            ResetColor,
            SetBackgroundColor(CANVAS)
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped(
            &format!(
                "c add  e edit  x remove  v draft  R submit +{}",
                comments.len()
            ),
            width as usize
        ))
    )
}

fn comments_at_line(
    comments: &[CodeReviewComment],
    change: &Change,
    hunk: &DiffHunk,
    diff_line: usize,
) -> usize {
    comments
        .iter()
        .filter(|comment| {
            comment.anchor.review.relative_path == change.path
                && comment.anchor.review.diff.as_bytes() == hunk.display()
                && comment.anchor.diff_line == diff_line
        })
        .count()
}

fn diff_visible_start(cursor: usize, offset: usize, available: usize) -> usize {
    if available == 0 || cursor < offset {
        cursor
    } else if cursor >= offset + available {
        cursor + 1 - available
    } else {
        offset
    }
}

fn reviewable_diff_line(display: &[u8]) -> usize {
    String::from_utf8_lossy(display)
        .lines()
        .position(|line| {
            (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"))
                || line.starts_with(' ')
        })
        .unwrap_or(0)
}

enum RunOutcome {
    Closed,
    AskAgent,
    ExpandReview,
    CollapseReview,
}

fn run() -> Result<RunOutcome, String> {
    let layout = Layout::from_environment()?;
    let repository = env::var_os("LANTERN_REPO").map(PathBuf::from).unwrap_or(
        env::current_dir().map_err(|cause| format!("cannot read current directory: {cause}"))?,
    );
    let rail = GitRail::open(&repository).map_err(|error| error.to_string())?;
    let mut state = State::load(&rail)?;
    let resume_path = env::var_os("LANTERN_GIT_RESUME_PATH")
        .map(PathBuf::from)
        .ok_or("Lantern Git resume path is not configured.")?;
    if resume_path.exists() {
        state.restore_review_context(&rail, &resume_path, layout == Layout::Review);
    }
    if let Some(path) = env::var_os("LANTERN_GIT_FOCUS_PATH").map(PathBuf::from) {
        state.focus_agent_changes(&path);
    }
    let (network_sender, network_receiver): (Sender<NetworkResult>, Receiver<NetworkResult>) =
        mpsc::channel();
    let (refresh_sender, refresh_receiver): (Sender<RefreshResult>, Receiver<RefreshResult>) =
        mpsc::channel();
    let mut refresh_in_flight = false;
    let mut next_refresh = Instant::now() + REFRESH_INTERVAL;
    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)
        .map_err(|cause| format!("cannot enter Git rail: {cause}"))?;
    let mut dirty = true;

    let outcome = loop {
        while let Ok(completed) = network_receiver.try_recv() {
            state.finish_network(&rail, completed);
            next_refresh = Instant::now() + REFRESH_INTERVAL;
            dirty = true;
        }
        while let Ok(completed) = refresh_receiver.try_recv() {
            refresh_in_flight = false;
            state.apply_background_refresh(completed);
            dirty = true;
        }
        if !refresh_in_flight && state.network.is_none() && Instant::now() >= next_refresh {
            request_background_refresh(
                &rail,
                state.repository_generation,
                state.refresh_focus(),
                &refresh_sender,
            );
            refresh_in_flight = true;
            next_refresh = Instant::now() + REFRESH_INTERVAL;
        }
        if dirty {
            draw(&mut stdout, &state).map_err(|cause| format!("cannot draw Git rail: {cause}"))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(50))
            .map_err(|cause| format!("cannot poll terminal: {cause}"))?
        {
            continue;
        }
        let input = event::read().map_err(|cause| format!("cannot read terminal: {cause}"))?;
        dirty = true;
        match input {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if state.network.is_some() {
                    if key.code == KeyCode::Esc {
                        state.cancel_network();
                    }
                    continue;
                }
                if state.help {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                        state.help = false;
                    }
                    continue;
                }
                if key.code == KeyCode::Char('?')
                    && !matches!(
                        &state.view,
                        View::Input { .. }
                            | View::ReviewInput { .. }
                            | View::ReviewSummary { .. }
                            | View::ReviewConfirm
                    )
                {
                    state.help = true;
                    continue;
                }
                match &mut state.view {
                    View::Changes => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break RunOutcome::Closed,
                        KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_selection(1),
                        KeyCode::Char(' ') => state.toggle_stage(&rail),
                        KeyCode::Enter
                            if state
                                .changes
                                .get(state.selected)
                                .is_some_and(|change| change.kind == ChangeKind::Conflicted) =>
                        {
                            state.open_selected_in_helix()
                        }
                        KeyCode::Enter | KeyCode::Char('d') => {
                            state.open_diff(&rail);
                            if layout == Layout::Compact && matches!(state.view, View::Diff { .. })
                            {
                                match state.publish_resume_context(&rail, &resume_path) {
                                    Ok(()) => break RunOutcome::ExpandReview,
                                    Err(message) => state.notice = Some(message),
                                }
                            }
                        }
                        KeyCode::Char('o') => state.open_selected_in_helix(),
                        KeyCode::Char('r') => state.refresh(&rail),
                        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            match state.export_review_context(&rail) {
                                Ok(()) => break RunOutcome::AskAgent,
                                Err(message) => state.notice = Some(message),
                            }
                        }
                        KeyCode::Char('a') => state.open_actions(),
                        _ => {}
                    },
                    View::Diff { .. } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') if layout == Layout::Review => {
                            match state.publish_resume_context(&rail, &resume_path) {
                                Ok(()) => break RunOutcome::CollapseReview,
                                Err(message) => state.notice = Some(message),
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') => state.view = View::Changes,
                        KeyCode::Up | KeyCode::Char('k') => state.move_diff_line(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_diff_line(1),
                        KeyCode::PageUp => state.move_diff_line(-10),
                        KeyCode::PageDown => state.move_diff_line(10),
                        KeyCode::Char('[') => state.move_hunk(-1),
                        KeyCode::Char(']') => state.move_hunk(1),
                        KeyCode::Char('p') => state.move_review_file(&rail, -1),
                        KeyCode::Char('n') => state.move_review_file(&rail, 1),
                        KeyCode::Char('c') => state.start_code_review_comment(&rail),
                        KeyCode::Char('e') => state.edit_comment_at_cursor(&rail),
                        KeyCode::Char('x') => state.delete_comment_at_cursor(&rail),
                        KeyCode::Char('v') if state.review_summary_len() > 0 => {
                            state.view = View::ReviewSummary { selected: 0 }
                        }
                        KeyCode::Char('R') if !state.review_comments.is_empty() => {
                            state.view = View::ReviewConfirm
                        }
                        KeyCode::Char(' ') => state.toggle_hunk(&rail),
                        KeyCode::Enter | KeyCode::Char('o') => state.open_selected_in_helix(),
                        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            match state.export_review_context(&rail) {
                                Ok(()) => break RunOutcome::AskAgent,
                                Err(message) => state.notice = Some(message),
                            }
                        }
                        _ => {}
                    },
                    View::Actions { .. } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => state.view = View::Changes,
                        KeyCode::Up | KeyCode::Char('k') => state.move_menu(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_menu(1),
                        KeyCode::Enter => state.choose_action(&rail, &network_sender),
                        _ => {}
                    },
                    View::Input { .. } => match key.code {
                        KeyCode::Esc => state.view = View::Actions { selected: 0 },
                        KeyCode::Enter => state.submit_input(&rail),
                        code if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            state.edit_input(code)
                        }
                        _ => {}
                    },
                    View::ReviewInput { .. } => match key.code {
                        KeyCode::Esc => {
                            let review = match &state.view {
                                View::ReviewInput { anchor, .. } => anchor.review.clone(),
                                _ => unreachable!(),
                            };
                            state.restore_review(&rail, &review);
                        }
                        KeyCode::Enter => state.submit_code_review_comment(&rail),
                        code if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            state.edit_input(code)
                        }
                        _ => {}
                    },
                    View::ReviewSummary { selected } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            let selected = *selected;
                            state.open_review_comment(&rail, selected);
                        }
                        KeyCode::Up | KeyCode::Char('k') => *selected = selected.saturating_sub(1),
                        KeyCode::Down | KeyCode::Char('j') => {
                            let len =
                                state.submitted_review_comments.len() + state.review_comments.len();
                            *selected = (*selected + 1).min(len.saturating_sub(1))
                        }
                        KeyCode::Enter => {
                            let selected = *selected;
                            state.open_review_comment(&rail, selected);
                        }
                        KeyCode::Char('e') => {
                            let index = *selected;
                            if index < state.submitted_review_comments.len() {
                                state.notice = Some(
                                    "Submitted feedback is read-only; add a new comment from the diff."
                                        .into(),
                                );
                                continue;
                            }
                            let index = index - state.submitted_review_comments.len();
                            let comment = state.review_comments[index].clone();
                            state.view = View::ReviewInput {
                                anchor: comment.anchor,
                                value: comment.comment,
                                editing: Some(index),
                            };
                        }
                        KeyCode::Char('x') => {
                            if *selected < state.submitted_review_comments.len() {
                                state.submitted_review_comments.remove(*selected);
                                let len = state.submitted_review_comments.len()
                                    + state.review_comments.len();
                                if len == 0 {
                                    state.view = View::Changes;
                                } else {
                                    *selected = (*selected).min(len - 1);
                                }
                                continue;
                            }
                            let draft_index = *selected - state.submitted_review_comments.len();
                            let removed = state.review_comments[draft_index].clone();
                            state.review_comments.remove(draft_index);
                            if state.review_comments.is_empty() {
                                if state.submitted_review_comments.is_empty() {
                                    state.restore_review(&rail, &removed.anchor.review);
                                    if let View::Diff { cursor, offset, .. } = &mut state.view {
                                        *cursor = removed.anchor.diff_line;
                                        *offset = removed.anchor.diff_line.saturating_sub(1);
                                    }
                                    state.notice = Some("Review draft is now empty.".into());
                                } else {
                                    let len = state.submitted_review_comments.len()
                                        + state.review_comments.len();
                                    *selected = (*selected).min(len - 1);
                                }
                            } else {
                                let len = state.submitted_review_comments.len()
                                    + state.review_comments.len();
                                *selected = (*selected).min(len - 1);
                            }
                        }
                        KeyCode::Char('R') if !state.review_comments.is_empty() => {
                            state.view = View::ReviewConfirm
                        }
                        _ => {}
                    },
                    View::ReviewConfirm => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            state.view = View::ReviewSummary { selected: 0 }
                        }
                        KeyCode::Enter | KeyCode::Char('y') => match state.submit_code_review() {
                            Ok(()) => break RunOutcome::Closed,
                            Err(message) => state.notice = Some(message),
                        },
                        _ => {}
                    },
                    View::Branches { .. } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            state.view = View::Actions { selected: 1 }
                        }
                        KeyCode::Up | KeyCode::Char('k') => state.move_menu(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_menu(1),
                        KeyCode::Enter => state.choose_branch(&rail),
                        _ => {}
                    },
                    View::History { .. } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            state.view = View::Actions { selected: 4 }
                        }
                        KeyCode::Up | KeyCode::Char('k') => state.move_menu(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_menu(1),
                        KeyCode::Enter => state.open_history_diff(&rail),
                        _ => {}
                    },
                    View::CommitDiff {
                        lines,
                        offset,
                        history,
                        selected,
                        ..
                    } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            state.view = View::History {
                                commits: std::mem::take(history),
                                selected: *selected,
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => *offset = offset.saturating_sub(1),
                        KeyCode::Down | KeyCode::Char('j') => {
                            *offset = (*offset + 1).min(lines.len().saturating_sub(1));
                        }
                        KeyCode::PageUp => *offset = offset.saturating_sub(10),
                        KeyCode::PageDown => {
                            *offset = (*offset + 10).min(lines.len().saturating_sub(1));
                        }
                        _ => {}
                    },
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(_) if state.help => {
                    state.help = false;
                }
                MouseEventKind::Down(MouseButton::Left) if matches!(state.view, View::Changes) => {
                    let row = mouse.row.saturating_sub(1) as usize;
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    let available = height.saturating_sub(3) as usize;
                    let index = visible_start(state.selected, available) + row;
                    if index < state.changes.len() && row < available {
                        if state.selected == index {
                            if state.changes[index].kind == ChangeKind::Conflicted {
                                state.open_selected_in_helix();
                            } else {
                                state.open_diff(&rail);
                                if layout == Layout::Compact
                                    && matches!(state.view, View::Diff { .. })
                                {
                                    match state.publish_resume_context(&rail, &resume_path) {
                                        Ok(()) => break RunOutcome::ExpandReview,
                                        Err(message) => state.notice = Some(message),
                                    }
                                }
                            }
                        } else {
                            state.selected = index;
                        }
                    }
                }
                MouseEventKind::Down(MouseButton::Right) if matches!(state.view, View::Changes) => {
                    let row = mouse.row.saturating_sub(1) as usize;
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    let available = height.saturating_sub(3) as usize;
                    let index = visible_start(state.selected, available) + row;
                    if index < state.changes.len() && row < available {
                        state.selected = index;
                        state.toggle_stage(&rail);
                    }
                }
                MouseEventKind::Down(MouseButton::Middle)
                    if matches!(state.view, View::Changes | View::Diff { .. }) =>
                {
                    state.open_selected_in_helix();
                }
                MouseEventKind::Down(MouseButton::Right)
                    if matches!(state.view, View::Diff { .. }) =>
                {
                    let row = mouse.row.saturating_sub(2) as usize;
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    let available = height.saturating_sub(3) as usize;
                    if let View::Diff { cursor, offset, .. } = &mut state.view {
                        let start = diff_visible_start(*cursor, *offset, available);
                        if row < available {
                            *cursor = start + row;
                        }
                    }
                    state.start_code_review_comment(&rail);
                }
                MouseEventKind::Down(MouseButton::Left)
                    if matches!(state.view, View::Diff { .. }) =>
                {
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    if mouse.row == height.saturating_sub(1) {
                        if !state.review_comments.is_empty() {
                            state.view = View::ReviewConfirm;
                        }
                    } else {
                        let row = mouse.row.saturating_sub(2) as usize;
                        let available = height.saturating_sub(3) as usize;
                        if let View::Diff { cursor, offset, .. } = &mut state.view {
                            let start = diff_visible_start(*cursor, *offset, available);
                            if row < available {
                                *cursor = start + row;
                            }
                        }
                    }
                }
                MouseEventKind::Down(MouseButton::Left)
                    if matches!(
                        state.view,
                        View::Actions { .. } | View::Branches { .. } | View::History { .. }
                    ) =>
                {
                    let row = mouse.row.saturating_sub(2) as usize;
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    let available = height.saturating_sub(4) as usize;
                    if row < available {
                        state.click_menu(row, available, &rail, &network_sender);
                    }
                }
                MouseEventKind::ScrollUp => match &mut state.view {
                    View::Changes => state.move_selection(-1),
                    View::Diff { .. } => state.move_diff_line(-3),
                    View::Actions { .. } | View::Branches { .. } | View::History { .. } => {
                        state.move_menu(-1)
                    }
                    View::CommitDiff { offset, .. } => *offset = offset.saturating_sub(3),
                    View::Input { .. }
                    | View::ReviewInput { .. }
                    | View::ReviewSummary { .. }
                    | View::ReviewConfirm => {}
                },
                MouseEventKind::ScrollDown => match &mut state.view {
                    View::Changes => state.move_selection(1),
                    View::Diff { .. } => state.move_diff_line(3),
                    View::Actions { .. } | View::Branches { .. } | View::History { .. } => {
                        state.move_menu(1)
                    }
                    View::CommitDiff { lines, offset, .. } => {
                        *offset = (*offset + 3).min(lines.len().saturating_sub(1));
                    }
                    View::Input { .. }
                    | View::ReviewInput { .. }
                    | View::ReviewSummary { .. }
                    | View::ReviewConfirm => {}
                },
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    };
    Ok(outcome)
}

fn main() {
    match run() {
        Ok(RunOutcome::Closed) => {}
        Ok(RunOutcome::AskAgent) => std::process::exit(ASK_AGENT_EXIT_CODE),
        Ok(RunOutcome::ExpandReview) => std::process::exit(EXPAND_REVIEW_EXIT_CODE),
        Ok(RunOutcome::CollapseReview) => std::process::exit(COLLAPSE_REVIEW_EXIT_CODE),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn git(repository: &Path, arguments: &[&str]) {
        assert!(
            Command::new("git")
                .args(arguments)
                .current_dir(repository)
                .status()
                .expect("run Git fixture command")
                .success()
        );
    }

    fn repository() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "lantern-git-refresh-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&root).expect("create refresh fixture");
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.name", "Lantern Test"]);
        git(&root, &["config", "user.email", "test@example.com"]);
        fs::write(
            root.join("tracked.txt"),
            "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n",
        )
        .expect("write tracked fixture");
        git(&root, &["add", "tracked.txt"]);
        git(&root, &["commit", "-qm", "initial"]);
        root
    }

    fn state_with(view: View) -> State {
        State {
            branch: "main".into(),
            changes: Vec::new(),
            selected: 0,
            view,
            notice: None,
            network: None,
            repository_generation: 0,
            help: false,
            review_comments: Vec::new(),
            submitted_review_comments: Vec::new(),
        }
    }

    #[test]
    fn orders_conflicts_before_reviewable_changes_without_duplicates() {
        let status = Status {
            branch: "main".into(),
            conflicted: vec![PathBuf::from("same.rs")],
            staged: vec![PathBuf::from("staged.rs"), PathBuf::from("same.rs")],
            unstaged: vec![PathBuf::from("work.rs")],
            untracked: vec![PathBuf::from("new.rs")],
        };
        let rows = changes(status);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].kind, ChangeKind::Conflicted);
        assert_eq!(rows[1].kind, ChangeKind::Staged);
        assert_eq!(rows[3].kind, ChangeKind::Untracked);
    }

    #[test]
    fn clips_narrow_rows_on_character_boundaries() {
        assert_eq!(clipped("hello", 5), "hello");
        assert_eq!(clipped("hélice", 4), "hél…");
        assert_eq!(clipped("hello", 1), "…");
        assert_eq!(clipped("hello", 0), "");
    }

    #[test]
    fn change_rows_expose_state_focus_and_the_filename_without_color() {
        let change = Change {
            kind: ChangeKind::Unstaged,
            path: PathBuf::from("src/deep/authentication/session_controller.ts"),
        };
        let row = change_row(&change, 34, true);
        assert!(row.starts_with("> modified "));
        assert!(row.ends_with("session_controller.ts"));
        assert!(row.chars().count() <= 34);
        assert_eq!(
            clipped_path(Path::new("src/deep/session.ts"), 12),
            "…/session.ts"
        );
    }

    #[test]
    fn preserves_separate_staged_and_unstaged_rows_for_one_file() {
        let status = Status {
            branch: "main".into(),
            staged: vec![PathBuf::from("both.rs")],
            unstaged: vec![PathBuf::from("both.rs")],
            ..Status::default()
        };
        let rows = changes(status);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, ChangeKind::Staged);
        assert_eq!(rows[1].kind, ChangeKind::Unstaged);
    }

    #[test]
    fn maps_mouse_rows_through_the_scrolled_window() {
        assert_eq!(visible_start(8, 4), 5);
        assert_eq!(visible_start(2, 4), 0);
    }

    #[test]
    fn action_navigation_is_bounded() {
        let mut state = state_with(View::Actions { selected: 0 });
        state.move_menu(-1);
        assert!(matches!(state.view, View::Actions { selected: 0 }));
        state.move_menu(99);
        assert!(matches!(state.view, View::Actions { selected: 4 }));
    }

    #[test]
    fn input_editing_preserves_unicode_boundaries() {
        let mut state = state_with(View::Input {
            kind: InputKind::Commit { staged: 1 },
            value: String::new(),
        });
        state.edit_input(KeyCode::Char('é'));
        state.edit_input(KeyCode::Backspace);
        assert!(matches!(state.view, View::Input { ref value, .. } if value.is_empty()));
    }

    #[test]
    fn one_shot_agent_focus_selects_the_first_still_changed_path() {
        let root = repository();
        fs::write(root.join("tracked.txt"), "agent edit\n").expect("edit tracked fixture");
        fs::write(root.join("second.txt"), "agent edit\n").expect("write second fixture");
        let rail = GitRail::open(&root).expect("open focus fixture");
        let mut state = State::load(&rail).expect("load focus fixture");
        state.selected = state
            .changes
            .iter()
            .position(|change| change.path == Path::new("tracked.txt"))
            .unwrap();
        state.open_diff(&rail);
        let review_comment = CodeReviewComment {
            anchor: state
                .code_review_anchor(&rail)
                .expect("review return anchor"),
            comment: "Preserve the public behavior.".into(),
        };
        state.view = View::Changes;
        let focus_path = root.join("agent-focus.json");
        fs::write(
            &focus_path,
            serde_json::to_vec(&AgentGitFocus {
                relative_paths: vec!["second.txt".into(), "tracked.txt".into()],
                review_comments: vec![review_comment.clone()],
            })
            .unwrap(),
        )
        .expect("write focus context");

        state.focus_agent_changes(&focus_path);

        assert!(!focus_path.exists());
        assert_eq!(state.changes[state.selected].path, Path::new("second.txt"));
        assert_eq!(state.submitted_review_comments, vec![review_comment]);
        assert_eq!(
            state.notice.as_deref(),
            Some("Reviewing 2 agent-edited files · Your review: 1 comment.")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn background_refresh_preserves_hunk_and_handles_a_cleaned_file() {
        let root = repository();
        let rail = GitRail::open(&root).expect("open refresh fixture");
        fs::write(
            root.join("tracked.txt"),
            "01\nchanged 02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n17\nchanged 18\n19\n20\n",
        )
        .expect("edit two hunks");
        let mut state = State::load(&rail).expect("load refresh state");
        let file_review = state.review_context(&rail).expect("file review context");
        assert_eq!(file_review.scope, GitReviewScope::File);
        assert!(file_review.diff.contains("changed 02"));
        assert!(file_review.diff.contains("changed 18"));
        state.open_diff(&rail);
        state.move_hunk(1);
        assert!(matches!(state.view, View::Diff { selected: 1, .. }));
        let review = state.review_context(&rail).expect("typed review context");
        assert_eq!(review.scope, GitReviewScope::Hunk);
        assert_eq!(review.state, GitReviewState::Modified);
        assert_eq!(review.relative_path, Path::new("tracked.txt"));
        assert!(review.diff.contains("changed 18"));
        assert!(!review.diff.contains("changed 02"));
        let draft = CodeReviewComment {
            anchor: CodeReviewAnchor {
                review: review.clone(),
                diff_line: 1,
                line: review.diff.lines().nth(1).unwrap().into(),
            },
            comment: "Keep this behavior explicit.".into(),
        };
        let selected_hunk = state.refresh_focus().expect("selected hunk").hunk_identity;

        fs::write(root.join("external.txt"), "external\n").expect("external edit");
        fs::write(
            root.join("tracked.txt"),
            "01\nchanged 02\n03\n04\n05\n06\n07\n08\n09\nchanged 10\n11\n12\n13\n14\n15\n16\n17\nchanged 18\n19\n20\n",
        )
        .expect("insert an earlier hunk");
        let (sender, receiver) = mpsc::channel();
        request_background_refresh(
            &rail,
            state.repository_generation,
            state.refresh_focus(),
            &sender,
        );
        state.apply_background_refresh(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("background refresh"),
        );
        assert!(state.changes.iter().any(|change| {
            change.kind == ChangeKind::Untracked && change.path == Path::new("external.txt")
        }));
        assert_eq!(
            state.refresh_focus().expect("preserved hunk").hunk_identity,
            selected_hunk
        );
        assert!(matches!(state.view, View::Diff { selected: 2, .. }));

        let resume = root.join("resume.json");
        fs::write(
            &resume,
            serde_json::to_vec(&GitResume {
                review: review.clone(),
                diff_line: 1,
                offset: 1,
                comments: vec![draft.clone()],
                submitted_comments: vec![draft.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        let mut restored = State::load(&rail).expect("reload Git rail");
        restored.restore_review_context(&rail, &resume, true);
        assert!(matches!(
            restored.view,
            View::Diff {
                selected: 2,
                cursor: 1,
                offset: 1,
                ..
            }
        ));
        assert_eq!(restored.review_comments, vec![draft.clone()]);
        assert_eq!(restored.submitted_review_comments, vec![draft.clone()]);
        assert!(!resume.exists());

        fs::write(
            &resume,
            serde_json::to_vec(&GitResume {
                review,
                diff_line: 1,
                offset: 1,
                comments: vec![draft.clone()],
                submitted_comments: vec![draft.clone()],
            })
            .unwrap(),
        )
        .unwrap();
        let mut collapsed = State::load(&rail).expect("reload compact Git rail");
        collapsed.restore_review_context(&rail, &resume, false);
        assert!(matches!(collapsed.view, View::Changes));
        assert_eq!(
            collapsed.changes[collapsed.selected].path,
            Path::new("tracked.txt")
        );
        assert_eq!(collapsed.review_comments, vec![draft]);
        assert_eq!(collapsed.submitted_review_comments.len(), 1);

        fs::write(
            root.join("tracked.txt"),
            "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n",
        )
        .expect("clean tracked file");
        request_background_refresh(
            &rail,
            state.repository_generation,
            state.refresh_focus(),
            &sender,
        );
        state.apply_background_refresh(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("clean refresh"),
        );
        assert!(matches!(state.view, View::Changes));
        assert!(
            state
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("is now clean"))
        );
        fs::remove_dir_all(root).expect("remove refresh fixture");
    }

    #[test]
    fn diff_cursor_creates_an_exact_line_review_anchor() {
        let root = repository();
        fs::write(
            root.join("tracked.txt"),
            "01\nchanged 02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n",
        )
        .expect("edit tracked fixture");
        let rail = GitRail::open(&root).expect("open review fixture");
        let mut state = State::load(&rail).expect("load review fixture");
        state.open_diff(&rail);

        let anchor = state.code_review_anchor(&rail).expect("exact line anchor");
        assert_eq!(anchor.review.relative_path, Path::new("tracked.txt"));
        assert_eq!(
            anchor.review.diff.lines().nth(anchor.diff_line),
            Some(anchor.line.as_str())
        );
        assert!(anchor.line.starts_with(['+', '-']));

        state.start_code_review_comment(&rail);
        state.submit_code_review_comment(&rail);
        assert!(state.review_comments.is_empty());
        assert!(
            state
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("must contain"))
        );
        state.edit_input(KeyCode::Char('f'));
        state.submit_code_review_comment(&rail);
        assert_eq!(state.review_comments.len(), 1);
        let View::Diff {
            change,
            hunks,
            selected,
            ..
        } = &state.view
        else {
            panic!("review input should return to the diff");
        };
        assert_eq!(
            comments_at_line(
                &state.review_comments,
                change,
                &hunks[*selected],
                anchor.diff_line
            ),
            1
        );
        state.edit_comment_at_cursor(&rail);
        assert!(matches!(
            state.view,
            View::ReviewInput {
                editing: Some(0),
                ..
            }
        ));
        state.edit_input(KeyCode::Char('x'));
        state.submit_code_review_comment(&rail);
        assert_eq!(state.review_comments[0].comment, "fx");
        state.delete_comment_at_cursor(&rail);
        assert!(state.review_comments.is_empty());
        assert!(matches!(state.view, View::Diff { .. }));
        fs::remove_dir_all(root).expect("remove review fixture");
    }
}
