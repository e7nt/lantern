use lantern_git_rail::Status;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileState {
    pub conflicted: bool,
    pub staged: bool,
    pub modified: bool,
    pub untracked: bool,
}

impl FileState {
    pub fn label(self) -> &'static str {
        match (self.conflicted, self.staged, self.modified, self.untracked) {
            (true, _, _, _) => "!",
            (_, true, true, _) => "±",
            (_, true, _, _) => "S",
            (_, _, true, _) => "M",
            (_, _, _, true) => "A",
            _ => "",
        }
    }

    fn merge(&mut self, other: Self) {
        self.conflicted |= other.conflicted;
        self.staged |= other.staged;
        self.modified |= other.modified;
        self.untracked |= other.untracked;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub path: PathBuf,
    pub depth: usize,
    pub directory: bool,
    pub expanded: bool,
    pub state: FileState,
    pub comments: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplorerTree {
    files: Vec<PathBuf>,
    status: HashMap<PathBuf, FileState>,
    comments: HashMap<PathBuf, usize>,
    expanded: BTreeSet<PathBuf>,
}

impl ExplorerTree {
    pub fn new(files: Vec<PathBuf>, status: &Status, comments: HashMap<PathBuf, usize>) -> Self {
        let mut states = HashMap::<PathBuf, FileState>::new();
        for path in &status.conflicted {
            states.entry(path.clone()).or_default().conflicted = true;
        }
        for path in &status.staged {
            states.entry(path.clone()).or_default().staged = true;
        }
        for path in &status.unstaged {
            states.entry(path.clone()).or_default().modified = true;
        }
        for path in &status.untracked {
            states.entry(path.clone()).or_default().untracked = true;
        }
        Self {
            files,
            status: states,
            comments,
            expanded: BTreeSet::new(),
        }
    }

    pub fn preserve_expansion(&mut self, previous: &Self) {
        self.expanded.clone_from(&previous.expanded);
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_owned());
        }
    }

    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
    }

    pub fn expand(&mut self, path: &Path) {
        self.expanded.insert(path.to_owned());
    }

    pub fn rows(&self) -> Vec<TreeRow> {
        let mut children = BTreeMap::<PathBuf, BTreeSet<(bool, PathBuf)>>::new();
        let mut directories = BTreeSet::new();
        for file in &self.files {
            let mut parent = PathBuf::new();
            let components = file.components().collect::<Vec<_>>();
            for (index, component) in components.iter().enumerate() {
                let path = parent.join(component.as_os_str());
                let directory = index + 1 < components.len();
                children
                    .entry(parent.clone())
                    .or_default()
                    .insert((!directory, path.clone()));
                if directory {
                    directories.insert(path.clone());
                }
                parent = path;
            }
        }
        let mut rows = Vec::new();
        self.append_rows(Path::new(""), 0, &children, &directories, &mut rows);
        rows
    }

    fn append_rows(
        &self,
        parent: &Path,
        depth: usize,
        children: &BTreeMap<PathBuf, BTreeSet<(bool, PathBuf)>>,
        directories: &BTreeSet<PathBuf>,
        rows: &mut Vec<TreeRow>,
    ) {
        let Some(entries) = children.get(parent) else {
            return;
        };
        for (_, path) in entries {
            let directory = directories.contains(path);
            let expanded = directory && self.expanded.contains(path);
            let (state, comments) = self.summary(path, directory);
            rows.push(TreeRow {
                path: path.clone(),
                depth,
                directory,
                expanded,
                state,
                comments,
            });
            if expanded {
                self.append_rows(path, depth + 1, children, directories, rows);
            }
        }
    }

    fn summary(&self, path: &Path, directory: bool) -> (FileState, usize) {
        if !directory {
            return (
                self.status.get(path).copied().unwrap_or_default(),
                self.comments.get(path).copied().unwrap_or_default(),
            );
        }
        let mut state = FileState::default();
        for (candidate, candidate_state) in &self.status {
            if candidate.starts_with(path) {
                state.merge(*candidate_state);
            }
        }
        let comments = self
            .comments
            .iter()
            .filter(|(candidate, _)| candidate.starts_with(path))
            .map(|(_, count)| count)
            .sum();
        (state, comments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_is_folder_first_and_propagates_change_and_comment_state() {
        let files = vec![
            "README.md".into(),
            "src/lib.rs".into(),
            "src/ui/tree.rs".into(),
        ];
        let status = Status {
            unstaged: vec!["src/ui/tree.rs".into()],
            ..Status::default()
        };
        let comments = HashMap::from([(PathBuf::from("src/ui/tree.rs"), 2)]);
        let mut tree = ExplorerTree::new(files, &status, comments);
        let rows = tree.rows();
        assert_eq!(rows[0].path, PathBuf::from("src"));
        assert_eq!(rows[0].state.label(), "M");
        assert_eq!(rows[0].comments, 2);
        assert_eq!(rows[1].path, PathBuf::from("README.md"));

        tree.toggle(Path::new("src"));
        tree.toggle(Path::new("src/ui"));
        let rows = tree.rows();
        assert!(
            rows.iter()
                .any(|row| row.path == Path::new("src/ui/tree.rs"))
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.path == Path::new("src/ui/tree.rs"))
                .expect("tree file")
                .comments,
            2
        );
    }

    #[test]
    fn combined_staged_and_modified_state_is_visible() {
        let status = Status {
            staged: vec!["src/lib.rs".into()],
            unstaged: vec!["src/lib.rs".into()],
            ..Status::default()
        };
        let tree = ExplorerTree::new(vec!["src/lib.rs".into()], &status, HashMap::new());
        let row = tree.rows().into_iter().next().expect("src directory");
        assert_eq!(row.state.label(), "±");
    }
}
