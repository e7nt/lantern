use lantern_git_rail_spike::GitRail;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run(repository: &Path, arguments: &[&str]) {
    assert!(
        Command::new("git")
            .args(arguments)
            .current_dir(repository)
            .status()
            .expect("run Git fixture command")
            .success(),
        "Git fixture command failed: {arguments:?}"
    );
}

fn run_fails(repository: &Path, arguments: &[&str]) {
    assert!(
        !Command::new("git")
            .args(arguments)
            .current_dir(repository)
            .status()
            .expect("run failing Git fixture command")
            .success()
    );
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "lantern-git-rail-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir(&root).expect("create fixture");
    run(&root, &["init", "-q", "-b", "main"]);
    run(&root, &["config", "user.name", "Lantern Test"]);
    run(&root, &["config", "user.email", "test@example.com"]);
    fs::write(root.join("tracked.txt"), "first\n").expect("write tracked file");
    run(&root, &["add", "tracked.txt"]);
    run(&root, &["commit", "-qm", "initial"]);
    root
}

#[test]
fn focused_git_journey_preserves_review_state() {
    let root = fixture();
    let rail = GitRail::open(&root).expect("open rail");
    fs::write(root.join("tracked.txt"), "first\nsecond\n").expect("change tracked file");
    fs::write(root.join("new.txt"), "new\n").expect("write untracked file");
    let status = rail.status().expect("read status");
    assert_eq!(status.branch, "main");
    assert_eq!(status.unstaged, [PathBuf::from("tracked.txt")]);
    assert_eq!(status.untracked, [PathBuf::from("new.txt")]);
    assert!(
        rail.diff(Path::new("tracked.txt"), false)
            .expect("diff")
            .starts_with(b"diff --git")
    );
    rail.stage(Path::new("tracked.txt")).expect("stage");
    assert_eq!(
        rail.status().expect("staged status").staged,
        [PathBuf::from("tracked.txt")]
    );
    rail.unstage(Path::new("tracked.txt")).expect("unstage");
    assert_eq!(
        rail.status().expect("unstaged status").unstaged,
        [PathBuf::from("tracked.txt")]
    );
    rail.stage(Path::new("tracked.txt")).expect("restage");
    rail.commit("update tracked file").expect("commit");
    rail.create_branch("review").expect("create branch");
    assert_eq!(rail.status().expect("branch status").branch, "review");
    rail.switch_branch("main").expect("switch branch");
    let commits = rail.recent_commits(2).expect("history");
    assert_eq!(commits[0].summary, "update tracked file");
    assert_eq!(commits[1].summary, "initial");
    assert_eq!(
        rail.status().expect("final status").untracked,
        [PathBuf::from("new.txt")]
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn rejects_repository_escape_and_invalid_branch() {
    let root = fixture();
    let rail = GitRail::open(&root).expect("open rail");
    fs::create_dir(root.join("nested")).expect("create nested directory");
    assert!(GitRail::open(root.join("nested")).is_err());
    assert!(rail.stage(Path::new("../outside")).is_err());
    assert!(rail.diff(Path::new("/outside"), false).is_err());
    assert!(rail.create_branch("-unsafe").is_err());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[cfg(unix)]
#[test]
fn preserves_non_utf8_git_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = fixture();
    let raw_name = OsString::from_vec(b"non-utf8-\xff.txt".to_vec());
    fs::write(root.join(&raw_name), "raw\n").expect("write non-UTF-8 path");
    let rail = GitRail::open(&root).expect("open rail");
    assert!(
        rail.status()
            .expect("raw status")
            .untracked
            .contains(&PathBuf::from(raw_name))
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn stages_and_unstages_one_hunk_without_touching_the_other() {
    let root = fixture();
    let rail = GitRail::open(&root).expect("open rail");
    fs::write(
        root.join("tracked.txt"),
        "first\nsecond\nthird\nfourth\nfifth\n",
    )
    .expect("establish multi-line base");
    run(&root, &["add", "tracked.txt"]);
    run(&root, &["commit", "-qm", "multi-line base"]);
    fs::write(
        root.join("tracked.txt"),
        "FIRST\nsecond\nthird\nfourth\nFIFTH\n",
    )
    .expect("write two hunks");
    let diff = Command::new("git")
        .args(["diff", "-U0", "--", "tracked.txt"])
        .current_dir(&root)
        .output()
        .expect("read two-hunk diff")
        .stdout;
    let first_marker = diff
        .windows(3)
        .position(|window| window == b"@@ ")
        .expect("first hunk");
    let second_marker = diff[first_marker + 3..]
        .windows(4)
        .position(|window| window == b"\n@@ ")
        .map(|offset| first_marker + 3 + offset + 1)
        .expect("second hunk");
    let first_hunk = &diff[..second_marker];

    rail.stage_hunk(first_hunk).expect("stage first hunk");
    let staged = String::from_utf8(
        rail.diff(Path::new("tracked.txt"), true)
            .expect("staged diff"),
    )
    .expect("UTF-8 staged diff");
    let unstaged = String::from_utf8(
        rail.diff(Path::new("tracked.txt"), false)
            .expect("unstaged diff"),
    )
    .expect("UTF-8 unstaged diff");
    assert!(staged.contains("+FIRST"));
    assert!(!staged.contains("+FIFTH"));
    assert!(unstaged.contains("+FIFTH"));
    assert!(!unstaged.contains("+FIRST"));

    rail.unstage_hunk(first_hunk).expect("unstage first hunk");
    assert!(rail.status().expect("unstaged status").staged.is_empty());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn exposes_conflicts_and_detached_head() {
    let root = fixture();
    let rail = GitRail::open(&root).expect("open rail");
    rail.create_branch("other").expect("create other branch");
    fs::write(root.join("tracked.txt"), "other\n").expect("write other change");
    run(&root, &["add", "tracked.txt"]);
    run(&root, &["commit", "-qm", "other change"]);
    rail.switch_branch("main").expect("return to main");
    fs::write(root.join("tracked.txt"), "main\n").expect("write main change");
    run(&root, &["add", "tracked.txt"]);
    run(&root, &["commit", "-qm", "main change"]);
    run_fails(&root, &["merge", "other"]);
    assert_eq!(
        rail.status().expect("conflicted status").conflicted,
        [PathBuf::from("tracked.txt")]
    );
    run(&root, &["merge", "--abort"]);
    run(&root, &["checkout", "--detach", "-q", "HEAD"]);
    assert!(
        rail.status()
            .expect("detached status")
            .branch
            .starts_with("(detached ")
    );
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn fetches_and_fast_forwards_without_hidden_merge() {
    let root = fixture();
    let suffix = root.file_name().expect("fixture name").to_string_lossy();
    let remote = root.with_file_name(format!("{suffix}-remote.git"));
    let peer = root.with_file_name(format!("{suffix}-peer"));
    fs::create_dir(&remote).expect("create remote directory");
    run(&remote, &["init", "--bare", "-q"]);
    run(
        &root,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("remote path"),
        ],
    );
    run(&root, &["push", "-qu", "origin", "main"]);
    assert!(
        Command::new("git")
            .args([
                "clone",
                "-q",
                "-b",
                "main",
                remote.to_str().expect("remote path"),
                peer.to_str().expect("peer path")
            ])
            .status()
            .expect("clone peer")
            .success()
    );
    run(&peer, &["config", "user.name", "Lantern Peer"]);
    run(&peer, &["config", "user.email", "peer@example.com"]);
    fs::write(peer.join("tracked.txt"), "remote\n").expect("write remote change");
    run(&peer, &["add", "tracked.txt"]);
    run(&peer, &["commit", "-qm", "remote change"]);
    run(&peer, &["push", "-q"]);

    let rail = GitRail::open(&root).expect("open rail");
    rail.fetch().expect("fetch");
    rail.pull_fast_forward().expect("fast-forward pull");
    assert_eq!(
        fs::read_to_string(root.join("tracked.txt")).expect("read pulled file"),
        "remote\n"
    );
    assert_eq!(
        rail.recent_commits(1).expect("history")[0].summary,
        "remote change"
    );

    fs::remove_dir_all(root).expect("remove fixture");
    fs::remove_dir_all(remote).expect("remove remote");
    fs::remove_dir_all(peer).expect("remove peer");
}
