use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

pub const MAX_DIFF_BYTES: usize = 512 * 1024;
pub const MAX_HISTORY_ENTRIES: usize = 50;

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

pub struct GitRail {
    repository: PathBuf,
}

impl GitRail {
    pub fn open(repository: impl AsRef<Path>) -> Result<Self, String> {
        let repository = repository
            .as_ref()
            .canonicalize()
            .map_err(|cause| format!("cannot open repository: {cause}"))?;
        let rail = Self { repository };
        let root = Path::new(rail.git_text(["rev-parse", "--show-toplevel"])?.trim())
            .canonicalize()
            .map_err(|cause| format!("cannot resolve Git root: {cause}"))?;
        if root != rail.repository {
            return Err("Git rail must open the exact workbench root".into());
        }
        Ok(rail)
    }

    pub fn status(&self) -> Result<Status, String> {
        Ok(Status {
            branch: self.branch()?,
            staged: self.paths(["diff", "--cached", "--name-only", "-z"])?,
            unstaged: self.paths(["diff", "--name-only", "-z"])?,
            untracked: self.paths(["ls-files", "--others", "--exclude-standard", "-z"])?,
            conflicted: self.paths(["diff", "--name-only", "--diff-filter=U", "-z"])?,
        })
    }

    pub fn diff(&self, path: &Path, staged: bool) -> Result<Vec<u8>, String> {
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

    pub fn untracked_diff(&self, path: &Path) -> Result<Vec<u8>, String> {
        validate_path(path)?;
        let tracked = self.paths(["ls-files", "--error-unmatch", "-z", "--"]);
        if tracked.is_ok_and(|paths| paths.iter().any(|candidate| candidate == path)) {
            return Err("untracked diff requires an untracked path".into());
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

    pub fn diff_hunks(&self, path: &Path, staged: bool) -> Result<Vec<DiffHunk>, String> {
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
        parse_diff_hunks(&diff)
    }

    pub fn stage(&self, path: &Path) -> Result<(), String> {
        self.path_command(["add", "--"], path)
    }
    pub fn unstage(&self, path: &Path) -> Result<(), String> {
        self.path_command(["restore", "--staged", "--"], path)
    }

    pub fn stage_hunk(&self, patch: &[u8]) -> Result<(), String> {
        self.apply_cached_patch(patch, false)
    }

    pub fn unstage_hunk(&self, patch: &[u8]) -> Result<(), String> {
        self.apply_cached_patch(patch, true)
    }

    pub fn create_branch(&self, name: &str) -> Result<(), String> {
        validate_branch(name)?;
        self.git_ok(["switch", "-c", name])
    }

    pub fn switch_branch(&self, name: &str) -> Result<(), String> {
        validate_branch(name)?;
        self.git_ok(["switch", name])
    }

    pub fn local_branches(&self) -> Result<Vec<String>, String> {
        let output = self.git_text([
            OsString::from("for-each-ref"),
            OsString::from("--sort=refname"),
            OsString::from("--format=%(refname:short)"),
            OsString::from("refs/heads"),
        ])?;
        Ok(output.lines().map(ToOwned::to_owned).collect())
    }

    pub fn commit(&self, message: &str) -> Result<(), String> {
        let message = message.trim();
        if message.is_empty() || message.len() > 4_096 || message.contains('\0') {
            return Err("commit message must contain 1 to 4096 bytes".into());
        }
        self.git_ok(["commit", "-m", message])
    }

    pub fn fetch(&self) -> Result<(), String> {
        self.git_ok(["fetch", "--prune"])
    }
    pub fn pull_fast_forward(&self) -> Result<(), String> {
        self.git_ok(["pull", "--ff-only"])
    }

    pub fn sync_state(&self) -> Result<SyncState, String> {
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
        parse_sync_counts(&counts)
    }

    pub fn recent_commits(&self, limit: usize) -> Result<Vec<Commit>, String> {
        if limit == 0 || limit > MAX_HISTORY_ENTRIES {
            return Err(format!("history limit must be 1 to {MAX_HISTORY_ENTRIES}"));
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

    pub fn commit_diff(&self, id: &str) -> Result<Vec<u8>, String> {
        if !(40..=64).contains(&id.len()) || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("commit id must be a full hexadecimal object id".into());
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

    fn paths<const N: usize>(&self, arguments: [&str; N]) -> Result<Vec<PathBuf>, String> {
        let output = self.git_bytes(arguments.map(OsString::from))?;
        output
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(path_from_git)
            .collect()
    }

    fn branch(&self) -> Result<String, String> {
        let symbolic = Command::new("git")
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .current_dir(&self.repository)
            .output()
            .map_err(|cause| format!("cannot read Git branch: {cause}"))?;
        if symbolic.status.success() {
            return String::from_utf8(symbolic.stdout)
                .map(|value| value.trim().to_owned())
                .map_err(|_| "Git branch is not UTF-8".into());
        }
        Ok(format!(
            "(detached {})",
            self.git_text(["rev-parse", "--short", "HEAD"])?.trim()
        ))
    }

    fn path_command<const N: usize>(&self, prefix: [&str; N], path: &Path) -> Result<(), String> {
        validate_path(path)?;
        let mut arguments = prefix.map(OsString::from).to_vec();
        arguments.push(path.as_os_str().to_owned());
        self.git_ok(arguments)
    }

    fn apply_cached_patch(&self, patch: &[u8], reverse: bool) -> Result<(), String> {
        if patch.is_empty() || patch.len() > MAX_DIFF_BYTES || !patch.starts_with(b"diff --git ") {
            return Err("Git hunk must be a bounded unified diff".into());
        }
        let mut command = Command::new("git");
        command.args(["apply", "--cached", "--recount", "--unidiff-zero"]);
        if reverse {
            command.arg("--reverse");
        }
        let mut child = command
            .current_dir(&self.repository)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|cause| format!("cannot run Git: {cause}"))?;
        child
            .stdin
            .take()
            .expect("Git stdin")
            .write_all(patch)
            .map_err(|cause| format!("cannot send Git hunk: {cause}"))?;
        let output = child
            .wait_with_output()
            .map_err(|cause| format!("cannot finish Git hunk: {cause}"))?;
        if !output.status.success() {
            return Err(git_failure(&output.stderr));
        }
        Ok(())
    }

    fn git_text<const N: usize, S: Into<OsString>>(
        &self,
        arguments: [S; N],
    ) -> Result<String, String> {
        String::from_utf8(self.git_bytes(arguments.map(Into::into))?)
            .map_err(|_| "Git output is not UTF-8".into())
    }

    fn git_bytes(&self, arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<u8>, String> {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(&self.repository)
            .output()
            .map_err(|cause| format!("cannot run Git: {cause}"))?;
        if !output.status.success() {
            return Err(git_failure(&output.stderr));
        }
        Ok(output.stdout)
    }

    fn git_bounded(
        &self,
        arguments: impl IntoIterator<Item = OsString>,
        limit: usize,
        accept_difference: bool,
    ) -> Result<Vec<u8>, String> {
        let mut child = Command::new("git")
            .args(arguments)
            .current_dir(&self.repository)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|cause| format!("cannot run Git: {cause}"))?;
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .expect("Git stdout")
            .take((limit + 1) as u64)
            .read_to_end(&mut output)
            .map_err(|cause| format!("cannot read Git output: {cause}"))?;
        if output.len() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Git diff exceeds the {limit}-byte display limit"));
        }
        let result = child
            .wait_with_output()
            .map_err(|cause| format!("cannot finish Git: {cause}"))?;
        if !(result.status.success() || accept_difference && result.status.code() == Some(1)) {
            return Err(git_failure(&result.stderr));
        }
        Ok(output)
    }

    fn git_ok(&self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<(), String> {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(&self.repository)
            .output()
            .map_err(|cause| format!("cannot run Git: {cause}"))?;
        if !output.status.success() {
            return Err(git_failure(&output.stderr));
        }
        Ok(())
    }
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

fn validate_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("Git path must be repository-relative without traversal".into());
    }
    Ok(())
}

fn validate_branch(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 255 || name.starts_with('-') || name.contains('\0') {
        return Err("branch name is invalid".into());
    }
    let output = Command::new("git")
        .args(["check-ref-format", "--branch", name])
        .output()
        .map_err(|cause| format!("cannot validate branch: {cause}"))?;
    if !output.status.success() {
        return Err("branch name is invalid".into());
    }
    Ok(())
}

fn git_failure(stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr);
    if detail.trim().is_empty() {
        "Git command failed without diagnostics".into()
    } else {
        format!("Git command failed: {}", detail.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
