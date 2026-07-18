use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

pub const MAX_DIFF_BYTES: usize = 512 * 1024;
pub const MAX_HISTORY_ENTRIES: usize = 50;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub branch: String,
    pub staged: Vec<PathBuf>,
    pub unstaged: Vec<PathBuf>,
    pub untracked: Vec<PathBuf>,
    pub conflicted: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub summary: String,
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
        let mut arguments = vec![OsString::from("diff"), OsString::from("--no-ext-diff")];
        if staged {
            arguments.push(OsString::from("--cached"));
        }
        arguments.extend([OsString::from("--"), path.as_os_str().to_owned()]);
        self.git_bounded(arguments, MAX_DIFF_BYTES)
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

    fn git_text<const N: usize>(&self, arguments: [&str; N]) -> Result<String, String> {
        String::from_utf8(self.git_bytes(arguments.map(OsString::from))?)
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
        if !result.status.success() {
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
