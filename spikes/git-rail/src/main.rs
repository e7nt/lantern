use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};
use lantern_git_rail_spike::{GitRail, Status};
use std::env;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::time::Duration;

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
    Diff { lines: Vec<String>, offset: usize },
}

struct State {
    branch: String,
    changes: Vec<Change>,
    selected: usize,
    view: View,
    notice: Option<String>,
}

impl State {
    fn load(rail: &GitRail) -> Result<Self, String> {
        let status = rail.status()?;
        Ok(Self {
            branch: status.branch.clone(),
            changes: changes(status),
            selected: 0,
            view: View::Changes,
            notice: None,
        })
    }

    fn refresh(&mut self, rail: &GitRail) {
        match rail.status() {
            Ok(status) => {
                self.branch = status.branch.clone();
                self.changes = changes(status);
                self.selected = self.selected.min(self.changes.len().saturating_sub(1));
                self.notice = None;
            }
            Err(message) => self.notice = Some(message),
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
            Err(message) => self.notice = Some(message),
        }
    }

    fn open_diff(&mut self, rail: &GitRail) {
        let Some(change) = self.changes.get(self.selected) else {
            return;
        };
        let diff = if change.kind == ChangeKind::Untracked {
            rail.untracked_diff(&change.path)
        } else {
            rail.diff(&change.path, change.kind.staged())
        };
        match diff {
            Ok(diff) => {
                let lines = String::from_utf8_lossy(&diff)
                    .lines()
                    .map(ToOwned::to_owned)
                    .collect();
                self.view = View::Diff { lines, offset: 0 };
                self.notice = None;
            }
            Err(message) => self.notice = Some(message),
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
        View::Diff { lines, offset } => draw_diff(stdout, lines, *offset, width, height)?,
    }
    stdout.flush()
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
        .unwrap_or("↵ diff  space stage  q quit");
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped(footer, width as usize))
    )
}

fn draw_diff(
    stdout: &mut Stdout,
    lines: &[String],
    offset: usize,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let available = height.saturating_sub(2) as usize;
    for (row, line) in lines.iter().skip(offset).take(available).enumerate() {
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
            MoveTo(0, row as u16 + 1),
            SetForegroundColor(color),
            Print(clipped(line, width as usize))
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        SetForegroundColor(MUTED),
        Print(clipped("Esc back  j/k scroll", width as usize))
    )
}

fn run() -> Result<(), String> {
    let repository = env::var_os("LANTERN_REPO").map(PathBuf::from).unwrap_or(
        env::current_dir().map_err(|cause| format!("cannot read current directory: {cause}"))?,
    );
    let rail = GitRail::open(&repository)?;
    let mut state = State::load(&rail)?;
    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)
        .map_err(|cause| format!("cannot enter Git rail: {cause}"))?;

    loop {
        draw(&mut stdout, &state).map_err(|cause| format!("cannot draw Git rail: {cause}"))?;
        if !event::poll(Duration::from_millis(250))
            .map_err(|cause| format!("cannot poll terminal: {cause}"))?
        {
            continue;
        }
        match event::read().map_err(|cause| format!("cannot read terminal: {cause}"))? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match &mut state.view {
                View::Changes => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => state.move_selection(1),
                    KeyCode::Char(' ') => state.toggle_stage(&rail),
                    KeyCode::Enter | KeyCode::Char('d') => state.open_diff(&rail),
                    KeyCode::Char('r') => state.refresh(&rail),
                    _ => {}
                },
                View::Diff { lines, offset } => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => state.view = View::Changes,
                    KeyCode::Up | KeyCode::Char('k') => *offset = offset.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        *offset = (*offset + 1).min(lines.len().saturating_sub(1));
                    }
                    _ => {}
                },
            },
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
                MouseEventKind::ScrollUp => match &mut state.view {
                    View::Changes => state.move_selection(-1),
                    View::Diff { offset, .. } => *offset = offset.saturating_sub(1),
                },
                MouseEventKind::ScrollDown => match &mut state.view {
                    View::Changes => state.move_selection(1),
                    View::Diff { lines, offset } => {
                        *offset = (*offset + 1).min(lines.len().saturating_sub(1))
                    }
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
}
