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
use lantern_git_rail_spike::{
    Cancellation, Commit, DiffHunk, GitRail, GitResult, Status, SyncState,
};
use std::env;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
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
    fn marker(self) -> char {
        match self {
            Self::Conflicted => '!',
            Self::Staged => '+',
            Self::Unstaged => '~',
            Self::Untracked => '?',
        }
    }

    fn staged(self) -> bool {
        self == Self::Staged
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
    },
    Actions {
        selected: usize,
    },
    Input {
        kind: InputKind,
        value: String,
    },
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
        } = &self.view
        else {
            return None;
        };
        hunks.get(*selected).map(|hunk| RefreshFocus {
            change: change.clone(),
            hunk_identity: hunk.display().to_vec(),
            selected: *selected,
            offset: *offset,
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
                self.view = View::Diff {
                    change: focus.change,
                    hunks,
                    selected,
                    offset: focus.offset.min(line_count.saturating_sub(1)),
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
                self.view = View::Diff {
                    change: change.clone(),
                    hunks,
                    selected: 0,
                    offset: 0,
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
            ..
        } = &mut self.view
        else {
            return;
        };
        *selected = selected
            .saturating_add_signed(amount)
            .min(hunks.len().saturating_sub(1));
        *offset = 0;
    }

    fn scroll_hunk(&mut self, amount: isize) {
        let View::Diff {
            hunks,
            selected,
            offset,
            ..
        } = &mut self.view
        else {
            return;
        };
        let line_count = hunks
            .get(*selected)
            .map(|hunk| String::from_utf8_lossy(hunk.display()).lines().count())
            .unwrap_or(0);
        *offset = offset
            .saturating_add_signed(amount)
            .min(line_count.saturating_sub(1));
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

    fn fail_action(&mut self, message: String) {
        self.view = View::Changes;
        self.notice = Some(message);
    }

    fn edit_input(&mut self, code: KeyCode) {
        let View::Input { kind, value } = &mut self.view else {
            return;
        };
        let limit = match kind {
            InputKind::Commit { .. } => 4_096,
            InputKind::CreateBranch => 255,
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
        } => draw_diff(stdout, change, hunks, *selected, *offset, width, height)?,
        View::Actions { selected } => {
            draw_menu(stdout, "Actions", &ACTIONS, *selected, width, height)?
        }
        View::Input { kind, value } => draw_input(stdout, *kind, value, width, height)?,
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
    if let Some(notice) = &state.notice {
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
            Print(clipped(item.as_ref(), width as usize)),
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
            Print(clipped(
                &format!("{} {}", change.kind.marker(), change.path.display()),
                width as usize,
            )),
            ResetColor,
            SetBackgroundColor(CANVAS)
        )?;
    }
    let footer = state
        .notice
        .as_deref()
        .unwrap_or("a actions  ↵ diff  space stage");
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
    selected: usize,
    offset: usize,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let Some(hunk) = hunks.get(selected) else {
        return Ok(());
    };
    queue!(
        stdout,
        MoveTo(0, 1),
        SetForegroundColor(MUTED),
        Print(clipped(
            &format!("{}/{} {}", selected + 1, hunks.len(), change.path.display()),
            width as usize,
        ))
    )?;
    let display = String::from_utf8_lossy(hunk.display());
    let available = height.saturating_sub(3) as usize;
    for (row, line) in display.lines().skip(offset).take(available).enumerate() {
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
        Print(clipped("↵ open  space stage  Pg scroll", width as usize))
    )
}

fn run() -> Result<(), String> {
    let repository = env::var_os("LANTERN_REPO").map(PathBuf::from).unwrap_or(
        env::current_dir().map_err(|cause| format!("cannot read current directory: {cause}"))?,
    );
    let rail = GitRail::open(&repository).map_err(|error| error.to_string())?;
    let mut state = State::load(&rail)?;
    let (network_sender, network_receiver): (Sender<NetworkResult>, Receiver<NetworkResult>) =
        mpsc::channel();
    let (refresh_sender, refresh_receiver): (Sender<RefreshResult>, Receiver<RefreshResult>) =
        mpsc::channel();
    let mut refresh_in_flight = false;
    let mut next_refresh = Instant::now() + REFRESH_INTERVAL;
    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)
        .map_err(|cause| format!("cannot enter Git rail: {cause}"))?;

    loop {
        while let Ok(completed) = network_receiver.try_recv() {
            state.finish_network(&rail, completed);
            next_refresh = Instant::now() + REFRESH_INTERVAL;
        }
        while let Ok(completed) = refresh_receiver.try_recv() {
            refresh_in_flight = false;
            state.apply_background_refresh(completed);
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
        draw(&mut stdout, &state).map_err(|cause| format!("cannot draw Git rail: {cause}"))?;
        if !event::poll(Duration::from_millis(50))
            .map_err(|cause| format!("cannot poll terminal: {cause}"))?
        {
            continue;
        }
        match event::read().map_err(|cause| format!("cannot read terminal: {cause}"))? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if state.network.is_some() {
                    if key.code == KeyCode::Esc {
                        state.cancel_network();
                    }
                    continue;
                }
                match &mut state.view {
                    View::Changes => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
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
                        KeyCode::Enter | KeyCode::Char('d') => state.open_diff(&rail),
                        KeyCode::Char('o') => state.open_selected_in_helix(),
                        KeyCode::Char('r') => state.refresh(&rail),
                        KeyCode::Char('a') => state.open_actions(),
                        _ => {}
                    },
                    View::Diff { .. } => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => state.view = View::Changes,
                        KeyCode::Up | KeyCode::Char('k') => state.move_hunk(-1),
                        KeyCode::Down | KeyCode::Char('j') => state.move_hunk(1),
                        KeyCode::PageUp => state.scroll_hunk(-10),
                        KeyCode::PageDown => state.scroll_hunk(10),
                        KeyCode::Char(' ') => state.toggle_hunk(&rail),
                        KeyCode::Enter | KeyCode::Char('o') => state.open_selected_in_helix(),
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
                MouseEventKind::Down(MouseButton::Left) if matches!(state.view, View::Changes) => {
                    let row = mouse.row.saturating_sub(1) as usize;
                    let (_, height) = terminal::size()
                        .map_err(|cause| format!("cannot read terminal size: {cause}"))?;
                    let available = height.saturating_sub(3) as usize;
                    let index = visible_start(state.selected, available) + row;
                    if index < state.changes.len() && row < available {
                        state.selected = index;
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
                    View::Diff { .. } => state.scroll_hunk(-3),
                    View::Actions { .. } | View::Branches { .. } | View::History { .. } => {
                        state.move_menu(-1)
                    }
                    View::CommitDiff { offset, .. } => *offset = offset.saturating_sub(3),
                    View::Input { .. } => {}
                },
                MouseEventKind::ScrollDown => match &mut state.view {
                    View::Changes => state.move_selection(1),
                    View::Diff { .. } => state.scroll_hunk(3),
                    View::Actions { .. } | View::Branches { .. } | View::History { .. } => {
                        state.move_menu(1)
                    }
                    View::CommitDiff { lines, offset, .. } => {
                        *offset = (*offset + 3).min(lines.len().saturating_sub(1));
                    }
                    View::Input { .. } => {}
                },
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
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
    fn background_refresh_preserves_hunk_and_handles_a_cleaned_file() {
        let root = repository();
        let rail = GitRail::open(&root).expect("open refresh fixture");
        fs::write(
            root.join("tracked.txt"),
            "01\nchanged 02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n17\nchanged 18\n19\n20\n",
        )
        .expect("edit two hunks");
        let mut state = State::load(&rail).expect("load refresh state");
        state.open_diff(&rail);
        state.move_hunk(1);
        assert!(matches!(state.view, View::Diff { selected: 1, .. }));
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
}
