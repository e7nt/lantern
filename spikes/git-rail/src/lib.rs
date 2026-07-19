use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

pub const MAX_DIFF_BYTES: usize = 512 * 1024;
pub const MAX_HISTORY_ENTRIES: usize = 50;
const MAX_COMMAND_BYTES: usize = 512 * 1024;
const MAX_ERROR_BYTES: usize = 8 * 1024;
const LOCAL_TIMEOUT: Duration = Duration::from_secs(5);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
const WAIT_INTERVAL: Duration = Duration::from_millis(5);

pub type GitResult<T> = Result<T, GitError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitErrorKind {
    InvalidInput,
    NotRepository,
    TimedOut,
    Cancelled,
    OutputTooLarge,
    AuthenticationRequired,
    CommandFailed,
    InvalidOutput,
    CannotStart,
}

#[derive(Debug, PartialEq, Eq)]
pub struct GitError {
    pub kind: GitErrorKind,
    operation: &'static str,
    detail: Option<String>,
}

impl GitError {
    fn new(kind: GitErrorKind, operation: &'static str, detail: Option<String>) -> Self {
        Self {
            kind,
            operation,
            detail,
        }
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let recovery = match self.kind {
            GitErrorKind::InvalidInput => "Correct the selected value and try again.",
            GitErrorKind::NotRepository => "Open the exact Git workbench root.",
            GitErrorKind::TimedOut => "Check the repository or remote, then retry.",
            GitErrorKind::Cancelled => "Retry when you are ready.",
            GitErrorKind::OutputTooLarge => "Review this change in Helix or the terminal.",
            GitErrorKind::AuthenticationRequired => {
                "Authenticate with Git in a terminal, then retry."
            }
            GitErrorKind::CommandFailed => "Run the operation in a terminal for full diagnostics.",
            GitErrorKind::InvalidOutput => "Check the installed Git version and repository state.",
            GitErrorKind::CannotStart => "Install Git or correct PATH, then retry.",
        };
        write!(formatter, "Git {} failed", self.operation)?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        write!(formatter, ". {recovery}")
    }
}

impl std::error::Error for GitError {}

impl From<String> for GitError {
    fn from(detail: String) -> Self {
        Self::new(
            GitErrorKind::InvalidOutput,
            "interpret output",
            Some(detail),
        )
    }
}

impl From<&str> for GitError {
    fn from(detail: &str) -> Self {
        detail.to_owned().into()
    }
}

#[derive(Debug)]
struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

struct RunOptions<'a> {
    operation: &'static str,
    timeout: Duration,
    stdout_limit: usize,
    accepted_exit_codes: &'a [i32],
    stdin: Option<&'a [u8]>,
    cancellation: Option<Cancellation>,
}

impl RunOptions<'_> {
    fn local(operation: &'static str) -> Self {
        Self {
            operation,
            timeout: LOCAL_TIMEOUT,
            stdout_limit: MAX_COMMAND_BYTES,
            accepted_exit_codes: &[0],
            stdin: None,
            cancellation: None,
        }
    }

    fn network(operation: &'static str) -> Self {
        Self {
            timeout: NETWORK_TIMEOUT,
            ..Self::local(operation)
        }
    }
}

#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: String,
    pub staged: Vec<PathBuf>,
    pub unstaged: Vec<PathBuf>,
    pub untracked: Vec<PathBuf>,
    pub conflicted: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    NoUpstream,
    UpToDate,
    Ahead { commits: usize },
    Behind { commits: usize },
    Diverged { ahead: usize, behind: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    patch: Vec<u8>,
    display: Vec<u8>,
    new_start: usize,
    new_lines: usize,
}

impl DiffHunk {
    pub fn patch(&self) -> &[u8] {
        &self.patch
    }

    pub fn display(&self) -> &[u8] {
        &self.display
    }

    pub fn navigation_lines(&self) -> Option<(usize, usize)> {
        if self.new_lines == 0 {
            return None;
        }
        let start = self.new_start.max(1);
        Some((start, start.saturating_add(self.new_lines)))
    }
}

#[derive(Clone)]
pub struct GitRail {
    repository: PathBuf,
}

impl GitRail {
    pub fn open(repository: impl AsRef<Path>) -> GitResult<Self> {
        let repository = repository.as_ref().canonicalize().map_err(|cause| {
            GitError::new(
                GitErrorKind::NotRepository,
                "open repository",
                Some(cause.to_string()),
            )
        })?;
        let rail = Self { repository };
        let root = Path::new(rail.git_text(["rev-parse", "--show-toplevel"])?.trim())
            .canonicalize()
            .map_err(|cause| {
                GitError::new(
                    GitErrorKind::NotRepository,
                    "resolve repository",
                    Some(cause.to_string()),
                )
            })?;
        if root != rail.repository {
            return Err(GitError::new(
                GitErrorKind::NotRepository,
                "open repository",
                Some("selected folder is not the exact workbench root".into()),
            ));
        }
        Ok(rail)
    }

    pub fn status(&self) -> GitResult<Status> {
        Ok(Status {
            branch: self.branch()?,
            staged: self.paths(["diff", "--cached", "--name-only", "-z"])?,
            unstaged: self.paths(["diff", "--name-only", "-z"])?,
            untracked: self.paths(["ls-files", "--others", "--exclude-standard", "-z"])?,
            conflicted: self.paths(["diff", "--name-only", "--diff-filter=U", "-z"])?,
        })
    }

    pub fn diff(&self, path: &Path, staged: bool) -> GitResult<Vec<u8>> {
        validate_path(path)?;
        let mut arguments = vec![
            OsString::from("diff"),
            OsString::from("--no-ext-diff"),
            OsString::from("--unified=0"),
        ];
        if staged {
            arguments.push(OsString::from("--cached"));
        }
        arguments.extend([OsString::from("--"), path.as_os_str().to_owned()]);
        self.git_bounded(arguments, MAX_DIFF_BYTES, false)
    }

    pub fn untracked_diff(&self, path: &Path) -> GitResult<Vec<u8>> {
        validate_path(path)?;
        let tracked = self.paths(["ls-files", "--error-unmatch", "-z", "--"]);
        if tracked.is_ok_and(|paths| paths.iter().any(|candidate| candidate == path)) {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "read untracked diff",
                Some("selected path is already tracked".into()),
            ));
        }
        self.git_bounded(
            [
                OsString::from("diff"),
                OsString::from("--no-index"),
                OsString::from("--no-ext-diff"),
                OsString::from("--unified=0"),
                OsString::from("--"),
                OsString::from(NULL_DEVICE),
                path.as_os_str().to_owned(),
            ],
            MAX_DIFF_BYTES,
            true,
        )
    }

    pub fn diff_hunks(&self, path: &Path, staged: bool) -> GitResult<Vec<DiffHunk>> {
        let diff = if staged {
            self.diff(path, true)?
        } else if self
            .paths(["ls-files", "--others", "--exclude-standard", "-z"])?
            .iter()
            .any(|candidate| candidate == path)
        {
            self.untracked_diff(path)?
        } else {
            self.diff(path, false)?
        };
        parse_diff_hunks(&diff).map_err(|detail| {
            GitError::new(GitErrorKind::InvalidOutput, "parse diff", Some(detail))
        })
    }

    pub fn stage(&self, path: &Path) -> GitResult<()> {
        self.path_command(["add", "--"], path)
    }
    pub fn unstage(&self, path: &Path) -> GitResult<()> {
        self.path_command(["restore", "--staged", "--"], path)
    }

    pub fn stage_hunk(&self, patch: &[u8]) -> GitResult<()> {
        self.apply_cached_patch(patch, false)
    }

    pub fn unstage_hunk(&self, patch: &[u8]) -> GitResult<()> {
        self.apply_cached_patch(patch, true)
    }

    pub fn create_branch(&self, name: &str) -> GitResult<()> {
        self.validate_branch(name)?;
        self.git_ok(["switch", "-c", name])
    }

    pub fn switch_branch(&self, name: &str) -> GitResult<()> {
        self.validate_branch(name)?;
        self.git_ok(["switch", name])
    }

    pub fn local_branches(&self) -> GitResult<Vec<String>> {
        let output = self.git_text([
            OsString::from("for-each-ref"),
            OsString::from("--sort=refname"),
            OsString::from("--format=%(refname:short)"),
            OsString::from("refs/heads"),
        ])?;
        Ok(output.lines().map(ToOwned::to_owned).collect())
    }

    pub fn commit(&self, message: &str) -> GitResult<()> {
        let message = message.trim();
        if message.is_empty() || message.len() > 4_096 || message.contains('\0') {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "commit",
                Some("message must contain 1 to 4096 bytes".into()),
            ));
        }
        self.git_ok(["commit", "-m", message])
    }

    pub fn fetch(&self) -> GitResult<()> {
        self.git_ok_with(["fetch", "--prune"], RunOptions::network("fetch"))
    }
    pub fn fetch_with_cancellation(&self, cancellation: Cancellation) -> GitResult<()> {
        let mut options = RunOptions::network("fetch");
        options.cancellation = Some(cancellation);
        self.git_ok_with(["fetch", "--prune"], options)
    }
    pub fn pull_fast_forward(&self) -> GitResult<()> {
        self.git_ok_with(
            ["pull", "--ff-only"],
            RunOptions::network("fast-forward pull"),
        )
    }
    pub fn pull_fast_forward_with_cancellation(&self, cancellation: Cancellation) -> GitResult<()> {
        let mut options = RunOptions::network("fast-forward pull");
        options.cancellation = Some(cancellation);
        self.git_ok_with(["pull", "--ff-only"], options)
    }

    pub fn sync_state(&self) -> GitResult<SyncState> {
        let branch = self.branch()?;
        if branch.starts_with("(detached ") {
            return Ok(SyncState::NoUpstream);
        }
        let upstream = self.git_text([
            OsString::from("for-each-ref"),
            OsString::from("--format=%(upstream)"),
            OsString::from(format!("refs/heads/{branch}")),
        ])?;
        let upstream = upstream.trim();
        if upstream.is_empty() {
            return Ok(SyncState::NoUpstream);
        }
        let counts = self.git_text([
            OsString::from("rev-list"),
            OsString::from("--left-right"),
            OsString::from("--count"),
            OsString::from(format!("HEAD...{upstream}")),
        ])?;
        parse_sync_counts(&counts).map_err(|detail| {
            GitError::new(
                GitErrorKind::InvalidOutput,
                "read synchronization state",
                Some(detail),
            )
        })
    }

    pub fn recent_commits(&self, limit: usize) -> GitResult<Vec<Commit>> {
        if limit == 0 || limit > MAX_HISTORY_ENTRIES {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "read history",
                Some(format!("limit must be 1 to {MAX_HISTORY_ENTRIES}")),
            ));
        }
        let output = self.git_bytes([
            OsString::from("log"),
            OsString::from(format!("-{limit}")),
            OsString::from("--format=%H%x00%s%x00"),
        ])?;
        output
            .split(|byte| *byte == 0)
            .collect::<Vec<_>>()
            .chunks(2)
            .filter(|pair| pair.len() == 2 && !pair[0].is_empty())
            .map(|pair| {
                Ok(Commit {
                    id: String::from_utf8(pair[0].to_vec())
                        .map_err(|_| "commit id is not UTF-8")?,
                    summary: String::from_utf8(pair[1].to_vec())
                        .map_err(|_| "commit summary is not UTF-8")?,
                })
            })
            .collect()
    }

    pub fn commit_diff(&self, id: &str) -> GitResult<Vec<u8>> {
        if !(40..=64).contains(&id.len()) || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "read commit diff",
                Some("commit id must be a full hexadecimal object id".into()),
            ));
        }
        self.git_bounded(
            [
                OsString::from("show"),
                OsString::from("--format="),
                OsString::from("--no-ext-diff"),
                OsString::from("--unified=0"),
                OsString::from(id),
                OsString::from("--"),
            ],
            MAX_DIFF_BYTES,
            false,
        )
    }

    fn paths<const N: usize>(&self, arguments: [&str; N]) -> GitResult<Vec<PathBuf>> {
        let output = self.git_bytes(arguments.map(OsString::from))?;
        output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| {
                path_from_git(path).map_err(|detail| {
                    GitError::new(GitErrorKind::InvalidOutput, "read paths", Some(detail))
                })
            })
            .collect()
    }

    fn branch(&self) -> GitResult<String> {
        let mut options = RunOptions::local("read branch");
        options.accepted_exit_codes = &[0, 1];
        let symbolic = self.run_git(["symbolic-ref", "--quiet", "--short", "HEAD"], options)?;
        if symbolic.status.success() {
            return String::from_utf8(symbolic.stdout)
                .map(|value| value.trim().to_owned())
                .map_err(|_| {
                    GitError::new(
                        GitErrorKind::InvalidOutput,
                        "read branch",
                        Some("branch name is not UTF-8".into()),
                    )
                });
        }
        Ok(format!(
            "(detached {})",
            self.git_text(["rev-parse", "--short", "HEAD"])?.trim()
        ))
    }

    fn path_command<const N: usize>(&self, prefix: [&str; N], path: &Path) -> GitResult<()> {
        validate_path(path)?;
        let mut arguments = prefix.map(OsString::from).to_vec();
        arguments.push(path.as_os_str().to_owned());
        self.git_ok(arguments)
    }

    fn apply_cached_patch(&self, patch: &[u8], reverse: bool) -> GitResult<()> {
        if patch.is_empty() || patch.len() > MAX_DIFF_BYTES || !patch.starts_with(b"diff --git ") {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "apply hunk",
                Some("the hunk is not a bounded unified diff".into()),
            ));
        }
        let mut arguments = vec!["apply", "--cached", "--recount", "--unidiff-zero"];
        if reverse {
            arguments.push("--reverse");
        }
        let mut options = RunOptions::local("apply hunk");
        options.stdin = Some(patch);
        options.stdout_limit = 0;
        self.run_git(arguments, options)?;
        Ok(())
    }

    fn git_text<const N: usize, S: Into<OsString>>(&self, arguments: [S; N]) -> GitResult<String> {
        String::from_utf8(self.git_bytes(arguments.map(Into::into))?).map_err(|_| {
            GitError::new(
                GitErrorKind::InvalidOutput,
                "read text",
                Some("output is not UTF-8".into()),
            )
        })
    }

    fn git_bytes(&self, arguments: impl IntoIterator<Item = OsString>) -> GitResult<Vec<u8>> {
        self.run_git(arguments, RunOptions::local("read repository"))
            .map(|output| output.stdout)
    }

    fn git_bounded(
        &self,
        arguments: impl IntoIterator<Item = OsString>,
        limit: usize,
        accept_difference: bool,
    ) -> GitResult<Vec<u8>> {
        let mut options = RunOptions::local("read diff");
        options.stdout_limit = limit;
        options.accepted_exit_codes = if accept_difference { &[0, 1] } else { &[0] };
        self.run_git(arguments, options).map(|output| output.stdout)
    }

    fn git_ok(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> GitResult<()> {
        self.git_ok_with(arguments, RunOptions::local("update repository"))
    }

    fn git_ok_with(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
        options: RunOptions<'_>,
    ) -> GitResult<()> {
        self.run_git(
            arguments
                .into_iter()
                .map(|argument| argument.as_ref().to_owned()),
            options,
        )?;
        Ok(())
    }

    fn validate_branch(&self, name: &str) -> GitResult<()> {
        if name.is_empty() || name.len() > 255 || name.starts_with('-') || name.contains('\0') {
            return Err(GitError::new(
                GitErrorKind::InvalidInput,
                "validate branch",
                Some("branch name is invalid".into()),
            ));
        }
        self.run_git(
            ["check-ref-format", "--branch", name],
            RunOptions::local("validate branch"),
        )?;
        Ok(())
    }

    fn run_git(
        &self,
        arguments: impl IntoIterator<Item = impl AsRef<OsStr>>,
        options: RunOptions<'_>,
    ) -> GitResult<GitOutput> {
        let mut command = Command::new("git");
        command
            .args(arguments)
            .current_dir(&self.repository)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "Never")
            .stdin(if options.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|cause| {
            GitError::new(
                GitErrorKind::CannotStart,
                options.operation,
                Some(cause.to_string()),
            )
        })?;

        let stdout = child.stdout.take().expect("piped Git stdout");
        let stderr = child.stderr.take().expect("piped Git stderr");
        let stdout_limit = options.stdout_limit;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, stdout_limit));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_ERROR_BYTES));
        let stdin_writer = options.stdin.map(|input| {
            let mut stdin = child.stdin.take().expect("piped Git stdin");
            let input = input.to_vec();
            thread::spawn(move || stdin.write_all(&input))
        });

        let deadline = Instant::now() + options.timeout;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|cause| {
                GitError::new(
                    GitErrorKind::CommandFailed,
                    options.operation,
                    Some(cause.to_string()),
                )
            })? {
                break status;
            }
            if Instant::now() >= deadline {
                terminate(&mut child);
                let _ = child.wait();
                join_reader(stdout_reader, options.operation)?;
                join_reader(stderr_reader, options.operation)?;
                if let Some(writer) = stdin_writer {
                    let _ = writer.join();
                }
                return Err(GitError::new(
                    GitErrorKind::TimedOut,
                    options.operation,
                    Some(format!("deadline was {} ms", options.timeout.as_millis())),
                ));
            }
            if options
                .cancellation
                .as_ref()
                .is_some_and(Cancellation::is_cancelled)
            {
                terminate(&mut child);
                let _ = child.wait();
                join_reader(stdout_reader, options.operation)?;
                join_reader(stderr_reader, options.operation)?;
                if let Some(writer) = stdin_writer {
                    let _ = writer.join();
                }
                return Err(GitError::new(
                    GitErrorKind::Cancelled,
                    options.operation,
                    None,
                ));
            }
            thread::sleep(WAIT_INTERVAL);
        };

        if let Some(writer) = stdin_writer {
            writer
                .join()
                .map_err(|_| {
                    GitError::new(
                        GitErrorKind::CommandFailed,
                        options.operation,
                        Some("stdin writer stopped unexpectedly".into()),
                    )
                })?
                .map_err(|cause| {
                    GitError::new(
                        GitErrorKind::CommandFailed,
                        options.operation,
                        Some(cause.to_string()),
                    )
                })?;
        }
        let (stdout, stdout_overflow) = join_reader(stdout_reader, options.operation)?;
        let (stderr, stderr_overflow) = join_reader(stderr_reader, options.operation)?;
        if stdout_overflow || stderr_overflow {
            return Err(GitError::new(
                GitErrorKind::OutputTooLarge,
                options.operation,
                Some("command output exceeded its display bound".into()),
            ));
        }
        if !status.success()
            && !status
                .code()
                .is_some_and(|code| options.accepted_exit_codes.contains(&code))
        {
            let kind = if authentication_failed(&stderr) {
                GitErrorKind::AuthenticationRequired
            } else {
                GitErrorKind::CommandFailed
            };
            return Err(GitError::new(
                kind,
                options.operation,
                status.code().map(|code| format!("exit status {code}")),
            ));
        }
        Ok(GitOutput { status, stdout })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut overflow = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let bytes = reader.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        let remaining = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..bytes.min(remaining)]);
        overflow |= bytes > remaining;
    }
    Ok((retained, overflow))
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

fn join_reader(
    reader: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>,
    operation: &'static str,
) -> GitResult<(Vec<u8>, bool)> {
    reader
        .join()
        .map_err(|_| {
            GitError::new(
                GitErrorKind::CommandFailed,
                operation,
                Some("output reader stopped unexpectedly".into()),
            )
        })?
        .map_err(|cause| {
            GitError::new(
                GitErrorKind::CommandFailed,
                operation,
                Some(cause.to_string()),
            )
        })
}

fn authentication_failed(stderr: &[u8]) -> bool {
    let detail = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    [
        "authentication failed",
        "terminal prompts disabled",
        "could not read username",
        "could not read password",
        "credential prompt",
    ]
    .iter()
    .any(|marker| detail.contains(marker))
}

fn parse_sync_counts(counts: &str) -> Result<SyncState, String> {
    let mut values = counts.split_whitespace();
    let ahead = values
        .next()
        .ok_or("Git sync state has no ahead count")?
        .parse::<usize>()
        .map_err(|_| "Git ahead count is invalid")?;
    let behind = values
        .next()
        .ok_or("Git sync state has no behind count")?
        .parse::<usize>()
        .map_err(|_| "Git behind count is invalid")?;
    if values.next().is_some() {
        return Err("Git sync state has unexpected fields".into());
    }
    Ok(match (ahead, behind) {
        (0, 0) => SyncState::UpToDate,
        (ahead, 0) => SyncState::Ahead { commits: ahead },
        (0, behind) => SyncState::Behind { commits: behind },
        (ahead, behind) => SyncState::Diverged { ahead, behind },
    })
}

fn parse_diff_hunks(diff: &[u8]) -> Result<Vec<DiffHunk>, String> {
    if !diff.starts_with(b"diff --git ") {
        return Err("Git did not return a unified file diff".into());
    }
    let line_starts = std::iter::once(0)
        .chain(
            diff.iter()
                .enumerate()
                .filter_map(|(index, byte)| (*byte == b'\n').then_some(index + 1)),
        )
        .filter(|start| *start < diff.len())
        .collect::<Vec<_>>();
    let hunk_starts = line_starts
        .into_iter()
        .filter(|start| diff[*start..].starts_with(b"@@ "))
        .collect::<Vec<_>>();
    let Some(first_hunk) = hunk_starts.first().copied() else {
        return Err("Git diff has no selectable text hunks".into());
    };
    let file_header = &diff[..first_hunk];
    let mut hunks = Vec::with_capacity(hunk_starts.len());
    for (index, start) in hunk_starts.iter().copied().enumerate() {
        let end = hunk_starts.get(index + 1).copied().unwrap_or(diff.len());
        let display = diff[start..end].to_vec();
        let header_end = display
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(display.len());
        let (new_start, new_lines) = parse_new_range(&display[..header_end])?;
        let mut patch = Vec::with_capacity(file_header.len() + display.len());
        patch.extend_from_slice(file_header);
        patch.extend_from_slice(&display);
        hunks.push(DiffHunk {
            patch,
            display,
            new_start,
            new_lines,
        });
    }
    Ok(hunks)
}

fn parse_new_range(header: &[u8]) -> Result<(usize, usize), String> {
    let plus = header
        .iter()
        .position(|byte| *byte == b'+')
        .ok_or("Git hunk has no new-file range")?;
    let range = &header[plus + 1..];
    let end = range
        .iter()
        .position(|byte| byte.is_ascii_whitespace())
        .unwrap_or(range.len());
    let range = std::str::from_utf8(&range[..end]).map_err(|_| "Git hunk range is not UTF-8")?;
    let (start, lines) = range.split_once(',').unwrap_or((range, "1"));
    let start = start
        .parse::<usize>()
        .map_err(|_| "Git hunk start line is invalid")?;
    let lines = lines
        .parse::<usize>()
        .map_err(|_| "Git hunk line count is invalid")?;
    Ok((start, lines))
}

#[cfg(unix)]
fn path_from_git(path: &[u8]) -> Result<PathBuf, String> {
    Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
fn path_from_git(path: &[u8]) -> Result<PathBuf, String> {
    String::from_utf8(path.to_vec())
        .map(PathBuf::from)
        .map_err(|_| "Git path is not UTF-8 on this platform".into())
}

fn validate_path(path: &Path) -> GitResult<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(GitError::new(
            GitErrorKind::InvalidInput,
            "validate path",
            Some("path must be repository-relative without traversal".into()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn repository() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lantern-git-runner-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&root).expect("create runner fixture");
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .current_dir(&root)
                .status()
                .expect("initialize runner fixture")
                .success()
        );
        root
    }

    #[test]
    fn splits_unified_diff_into_independently_applicable_hunks() {
        let diff = b"diff --git a/a.txt b/a.txt\nindex 123..456 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n@@ -5,0 +6,2 @@\n+six\n+seven\n";
        let hunks = parse_diff_hunks(diff).expect("parse hunks");
        assert_eq!(hunks.len(), 2);
        assert!(hunks[0].patch().starts_with(b"diff --git "));
        assert!(!hunks[0].patch().windows(6).any(|value| value == b"+seven"));
        assert_eq!(hunks[0].navigation_lines(), Some((1, 2)));
        assert_eq!(hunks[1].navigation_lines(), Some((6, 8)));
    }

    #[test]
    fn deletion_only_hunk_has_no_fabricated_current_range() {
        let diff = b"diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +0,0 @@\n-old\n";
        let hunks = parse_diff_hunks(diff).expect("parse deletion");
        assert_eq!(hunks[0].navigation_lines(), None);
    }

    #[test]
    fn rejects_binary_or_malformed_diffs_without_a_hunk() {
        assert!(parse_diff_hunks(b"diff --git a/a b/a\nBinary files differ\n").is_err());
        assert!(parse_diff_hunks(b"not a diff\n").is_err());
    }

    #[test]
    fn classifies_upstream_counts_without_prose_parsing() {
        assert_eq!(parse_sync_counts("0\t0\n").unwrap(), SyncState::UpToDate);
        assert_eq!(
            parse_sync_counts("2 0").unwrap(),
            SyncState::Ahead { commits: 2 }
        );
        assert_eq!(
            parse_sync_counts("0 3").unwrap(),
            SyncState::Behind { commits: 3 }
        );
        assert_eq!(
            parse_sync_counts("2 3").unwrap(),
            SyncState::Diverged {
                ahead: 2,
                behind: 3
            }
        );
        assert!(parse_sync_counts("unknown").is_err());
    }

    #[test]
    fn runner_enforces_noninteractive_git_environment() {
        let root = repository();
        let rail = GitRail::open(&root).expect("open runner fixture");
        rail.run_git(
            [
                "-c",
                "alias.check=!test \"$GIT_TERMINAL_PROMPT\" = 0 && test \"$GCM_INTERACTIVE\" = Never",
                "check",
            ],
            RunOptions::local("check environment"),
        )
        .expect("noninteractive environment");
        fs::remove_dir_all(root).expect("remove runner fixture");
    }

    #[test]
    fn runner_bounds_output_and_deadlines_the_process_group() {
        let root = repository();
        let rail = GitRail::open(&root).expect("open runner fixture");
        let mut bounded = RunOptions::local("bound output");
        bounded.stdout_limit = 8;
        let error = rail
            .run_git(["-c", "alias.emit=!printf 123456789", "emit"], bounded)
            .expect_err("reject oversized output");
        assert_eq!(error.kind, GitErrorKind::OutputTooLarge);

        let mut deadline = RunOptions::local("deadline probe");
        deadline.timeout = Duration::from_millis(25);
        let started = Instant::now();
        let error = rail
            .run_git(["-c", "alias.pause=!sleep 2", "pause"], deadline)
            .expect_err("time out process group");
        assert_eq!(error.kind, GitErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_millis(500));
        fs::remove_dir_all(root).expect("remove runner fixture");
    }

    #[test]
    fn runner_cancels_the_process_group_before_its_deadline() {
        let root = repository();
        let rail = GitRail::open(&root).expect("open runner fixture");
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            signal.cancel();
        });
        let mut options = RunOptions::network("cancel operation");
        options.cancellation = Some(cancellation);
        let started = Instant::now();
        let error = rail
            .run_git(["-c", "alias.pause=!sleep 2", "pause"], options)
            .expect_err("cancel process group");
        assert_eq!(error.kind, GitErrorKind::Cancelled);
        assert!(started.elapsed() < Duration::from_millis(500));
        fs::remove_dir_all(root).expect("remove runner fixture");
    }

    #[test]
    fn authentication_errors_are_typed_without_echoing_provider_output() {
        assert!(authentication_failed(b"fatal: terminal prompts disabled"));
        let error = GitError::new(GitErrorKind::AuthenticationRequired, "fetch", None);
        assert!(!error.to_string().contains("fatal"));
        assert!(error.to_string().contains("Authenticate with Git"));
    }
}
