use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use crossterm::style::{Color, Print, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{execute, queue};
use lantern_explorer::{ExplorerTree, TreeRow};
use lantern_git_rail::GitRail;
use lantern_protocol::{AgentGitFocus, MAX_AGENT_GIT_FOCUS_BYTES, validate_agent_git_focus};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Stdout, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
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
const REFRESH_INTERVAL: Duration = Duration::from_millis(750);

struct Snapshot {
    tree: ExplorerTree,
}

struct State {
    tree: ExplorerTree,
    selected_path: Option<PathBuf>,
    selected: usize,
    offset: usize,
    notice: Option<String>,
    help: bool,
}

impl State {
    fn apply(&mut self, mut snapshot: Snapshot) {
        snapshot.tree.preserve_expansion(&self.tree);
        self.tree = snapshot.tree;
        let rows = self.tree.rows();
        self.selected = self
            .selected_path
            .as_ref()
            .and_then(|path| rows.iter().position(|row| &row.path == path))
            .unwrap_or_else(|| self.selected.min(rows.len().saturating_sub(1)));
        self.remember_selection(&rows);
    }

    fn move_selection(&mut self, amount: isize) {
        let rows = self.tree.rows();
        if rows.is_empty() {
            return;
        }
        self.selected = self
            .selected
            .saturating_add_signed(amount)
            .min(rows.len() - 1);
        self.remember_selection(&rows);
    }

    fn remember_selection(&mut self, rows: &[TreeRow]) {
        self.selected_path = rows.get(self.selected).map(|row| row.path.clone());
    }

    fn selected_row(&self) -> Option<TreeRow> {
        self.tree.rows().get(self.selected).cloned()
    }

    fn activate(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_row() else {
            return Ok(());
        };
        if row.directory {
            self.tree.toggle(&row.path);
            return Ok(());
        }
        open_file(&row.path)
    }

    fn collapse(&mut self) {
        let Some(row) = self.selected_row() else {
            return;
        };
        if row.directory && row.expanded {
            self.tree.collapse(&row.path);
            return;
        }
        let Some(parent) = row
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        else {
            return;
        };
        let rows = self.tree.rows();
        if let Some(index) = rows.iter().position(|candidate| candidate.path == parent) {
            self.selected = index;
            self.remember_selection(&rows);
        }
    }

    fn expand(&mut self) -> Result<(), String> {
        let Some(row) = self.selected_row() else {
            return Ok(());
        };
        if row.directory {
            self.tree.expand(&row.path);
            Ok(())
        } else {
            open_file(&row.path)
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
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

fn review_counts(path: Option<&Path>) -> HashMap<PathBuf, usize> {
    let Some(path) = path else {
        return HashMap::new();
    };
    let Ok(bytes) = fs::read(path) else {
        return HashMap::new();
    };
    if bytes.len() > MAX_AGENT_GIT_FOCUS_BYTES + 128 * 1024 {
        return HashMap::new();
    }
    let Ok(focus) = serde_json::from_slice::<AgentGitFocus>(&bytes) else {
        return HashMap::new();
    };
    if validate_agent_git_focus(&focus).is_err() {
        return HashMap::new();
    }
    let mut counts = HashMap::new();
    for comment in focus.review_comments {
        *counts
            .entry(comment.anchor.review.relative_path)
            .or_insert(0) += 1;
    }
    counts
}

fn load_snapshot(rail: &GitRail, focus_path: Option<&Path>) -> Result<Snapshot, String> {
    let files = rail.workspace_files().map_err(|error| error.to_string())?;
    let status = rail.status().map_err(|error| error.to_string())?;
    Ok(Snapshot {
        tree: ExplorerTree::new(files, &status, review_counts(focus_path)),
    })
}

fn request_refresh(
    rail: GitRail,
    focus_path: Option<PathBuf>,
    sender: Sender<Result<Snapshot, String>>,
) {
    thread::spawn(move || {
        let _ = sender.send(load_snapshot(&rail, focus_path.as_deref()));
    });
}

fn open_file(path: &Path) -> Result<(), String> {
    let open = env::var_os("LANTERN_OPEN_BIN").ok_or("Lantern file opener is not configured.")?;
    let status = Command::new(open)
        .arg(path)
        .args(["1", "1", "1", "2"])
        .status()
        .map_err(|cause| format!("Cannot open {}: {cause}", path.display()))?;
    if !status.success() {
        return Err(format!("Could not open {} in Helix.", path.display()));
    }
    focus_editor()
}

fn focus_editor() -> Result<(), String> {
    let pane =
        env::var_os("LANTERN_EDITOR_PANE").ok_or("Lantern editor pane is not configured.")?;
    let status = Command::new("tmux")
        .args(["select-pane", "-t"])
        .arg(pane)
        .status()
        .map_err(|cause| format!("Cannot focus Helix: {cause}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "Could not focus Helix.".into())
}

fn fit(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn draw(stdout: &mut Stdout, state: &mut State) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let width = usize::from(width);
    let body_height = usize::from(height.saturating_sub(2));
    let rows = state.tree.rows();
    if state.selected < state.offset {
        state.offset = state.selected;
    } else if state.selected >= state.offset.saturating_add(body_height) {
        state.offset = state.selected.saturating_sub(body_height.saturating_sub(1));
    }
    queue!(
        stdout,
        SetBackgroundColor(CANVAS),
        SetForegroundColor(TEXT),
        Clear(ClearType::All),
        MoveTo(1, 0),
        SetForegroundColor(ACCENT),
        Print(fit("WORKBENCH", width.saturating_sub(2)))
    )?;
    if state.help {
        for (row, line) in [
            "↑↓/jk  move",
            "Enter   open/toggle",
            "←/→     collapse/expand",
            "click   open/toggle",
            "r       refresh",
            "Esc     focus editor",
            "?       close help",
        ]
        .iter()
        .enumerate()
        {
            if row + 2 >= usize::from(height) {
                break;
            }
            queue!(
                stdout,
                MoveTo(1, (row + 2) as u16),
                Print(fit(line, width.saturating_sub(2)))
            )?;
        }
    } else {
        for (screen_row, item) in rows.iter().skip(state.offset).take(body_height).enumerate() {
            let absolute = state.offset + screen_row;
            let y = (screen_row + 1) as u16;
            let marker = if item.directory {
                if item.expanded { "▾" } else { "▸" }
            } else {
                " "
            };
            let name = item
                .path
                .file_name()
                .unwrap_or(item.path.as_os_str())
                .to_string_lossy();
            let suffix = match (item.state.label(), item.comments) {
                ("", 0) => String::new(),
                (state, 0) => format!("  {state}"),
                ("", comments) => format!("  ·{comments}"),
                (state, comments) => format!("  {state} ·{comments}"),
            };
            let indent = "  ".repeat(item.depth);
            let label = fit(
                &format!("{indent}{marker} {name}{suffix}"),
                width.saturating_sub(2),
            );
            if absolute == state.selected {
                queue!(
                    stdout,
                    SetBackgroundColor(SELECTED),
                    SetForegroundColor(TEXT)
                )?;
            } else if item.state.label().is_empty() && item.comments == 0 {
                queue!(stdout, SetBackgroundColor(CANVAS), SetForegroundColor(TEXT))?;
            } else {
                queue!(
                    stdout,
                    SetBackgroundColor(CANVAS),
                    SetForegroundColor(ACCENT)
                )?;
            }
            queue!(stdout, MoveTo(1, y), Print(label))?;
        }
    }
    let footer = state.notice.as_deref().unwrap_or("? help · Esc editor");
    queue!(
        stdout,
        SetBackgroundColor(CANVAS),
        SetForegroundColor(MUTED),
        MoveTo(1, height.saturating_sub(1)),
        Print(fit(footer, width.saturating_sub(2)))
    )?;
    stdout.flush()
}

fn run() -> Result<(), String> {
    let repository = env::var_os("LANTERN_REPO")
        .map(PathBuf::from)
        .ok_or("Lantern repository is not configured.")?;
    let focus_path = env::var_os("LANTERN_GIT_FOCUS_PATH").map(PathBuf::from);
    let rail = GitRail::open(repository).map_err(|error| error.to_string())?;
    let snapshot = load_snapshot(&rail, focus_path.as_deref())?;
    let mut state = State {
        tree: snapshot.tree,
        selected_path: None,
        selected: 0,
        offset: 0,
        notice: None,
        help: false,
    };
    let (sender, receiver) = mpsc::channel::<Result<Snapshot, String>>();
    let mut refresh_in_flight = false;
    let mut next_refresh = Instant::now() + REFRESH_INTERVAL;
    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)
        .map_err(|cause| format!("Cannot enter the explorer: {cause}"))?;
    let mut dirty = true;
    loop {
        if let Ok(result) = receiver.try_recv() {
            refresh_in_flight = false;
            match result {
                Ok(snapshot) => state.apply(snapshot),
                Err(message) => state.notice = Some(message),
            }
            dirty = true;
        }
        if !refresh_in_flight && Instant::now() >= next_refresh {
            request_refresh(rail.clone(), focus_path.clone(), sender.clone());
            refresh_in_flight = true;
            next_refresh = Instant::now() + REFRESH_INTERVAL;
        }
        if dirty {
            draw(&mut stdout, &mut state)
                .map_err(|cause| format!("Cannot draw explorer: {cause}"))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(50)).map_err(|cause| cause.to_string())? {
            continue;
        }
        dirty = true;
        match event::read().map_err(|cause| cause.to_string())? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if state.help {
                    state.help = false;
                    continue;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => state.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => state.move_selection(1),
                    KeyCode::Left | KeyCode::Char('h') => state.collapse(),
                    KeyCode::Right | KeyCode::Char('l') => {
                        if let Err(message) = state.expand() {
                            state.notice = Some(message);
                        }
                    }
                    KeyCode::Enter => {
                        if let Err(message) = state.activate() {
                            state.notice = Some(message);
                        }
                    }
                    KeyCode::Esc => {
                        if let Err(message) = focus_editor() {
                            state.notice = Some(message);
                        }
                    }
                    KeyCode::Char('?') => state.help = true,
                    KeyCode::Char('r') => next_refresh = Instant::now(),
                    KeyCode::Char('q') => break,
                    _ => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => state.move_selection(-3),
                MouseEventKind::ScrollDown => state.move_selection(3),
                MouseEventKind::Down(MouseButton::Left) if mouse.row > 0 => {
                    let index = state.offset + usize::from(mouse.row - 1);
                    let rows = state.tree.rows();
                    if index < rows.len() {
                        state.selected = index;
                        state.remember_selection(&rows);
                        if let Err(message) = state.activate() {
                            state.notice = Some(message);
                        }
                    }
                }
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
