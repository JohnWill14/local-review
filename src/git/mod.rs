use anyhow::{anyhow, Context, Result};
use git2::{BranchType, DiffFormat, DiffOptions, Repository};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const TAB_WIDTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRef {
    pub name: String,
    pub kind: RefKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Local,
    Remote,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Typechange,
    Other,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub origin: char,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ReviewFile {
    pub path: String,
    pub status: ChangeKind,
    pub additions: u32,
    pub deletions: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub files: Vec<ReviewFile>,
    pub base: String,
    pub head: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub title: String,
}

pub struct RepositoryModel {
    repo: Repository,
    pub root: PathBuf,
}

impl RepositoryModel {
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let start = path.unwrap_or_else(|| Path::new("."));
        let repo = Repository::discover(start)
            .with_context(|| format!("Git repository not found at {}", start.display()))?;
        let root = repo
            .workdir()
            .or_else(|| repo.path().parent())
            .ok_or_else(|| anyhow!("repository has no working directory"))?
            .to_path_buf();
        Ok(Self { repo, root })
    }

    pub fn display_name(&self) -> String {
        self.root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repository")
            .to_owned()
    }

    pub fn current_ref(&self) -> Option<String> {
        self.repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().map(str::to_owned))
    }

    pub fn default_base(&self) -> Option<String> {
        ["main", "master", "origin/main", "origin/master"]
            .into_iter()
            .find(|name| self.repo.revparse_single(name).is_ok())
            .map(str::to_owned)
    }

    pub fn refs(&self) -> Result<Vec<GitRef>> {
        let mut refs = Vec::new();
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                refs.push(GitRef {
                    name: name.to_owned(),
                    kind: RefKind::Local,
                });
            }
        }
        for branch in self.repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                refs.push(GitRef {
                    name: name.to_owned(),
                    kind: RefKind::Remote,
                });
            }
        }
        let tags = self.repo.tag_names(None)?;
        for tag in tags.iter().flatten() {
            refs.push(GitRef {
                name: tag.to_owned(),
                kind: RefKind::Tag,
            });
        }
        refs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(refs)
    }

    pub fn comparison(&self, base: &str, head: &str, include_worktree: bool) -> Result<Comparison> {
        let base_commit = self
            .repo
            .revparse_single(base)
            .with_context(|| format!("invalid base reference: {base}"))?
            .peel_to_commit()?;
        let head_commit = self
            .repo
            .revparse_single(head)
            .with_context(|| format!("invalid head reference: {head}"))?
            .peel_to_commit()?;
        let merge_base = self
            .repo
            .merge_base(base_commit.id(), head_commit.id())
            .unwrap_or(base_commit.id());
        let base_tree = self.repo.find_commit(merge_base)?.tree()?;
        let head_tree = head_commit.tree()?;
        let mut files = BTreeMap::new();
        self.collect_diff(
            self.repo.diff_tree_to_tree(
                Some(&base_tree),
                Some(&head_tree),
                Some(&mut diff_options()),
            )?,
            &mut files,
        )?;
        if include_worktree {
            self.collect_diff(
                self.repo
                    .diff_tree_to_workdir_with_index(Some(&head_tree), Some(&mut diff_options()))?,
                &mut files,
            )?;
        }
        Ok(Comparison {
            files: files.into_values().collect(),
            base: base.to_owned(),
            head: head.to_owned(),
        })
    }

    pub fn commits_between(&self, base: &str, head: &str) -> Result<Vec<CommitInfo>> {
        let base_commit = self.repo.revparse_single(base)?.peel_to_commit()?;
        let head_commit = self.repo.revparse_single(head)?.peel_to_commit()?;
        let merge_base = self
            .repo
            .merge_base(base_commit.id(), head_commit.id())
            .unwrap_or(base_commit.id());
        let mut walk = self.repo.revwalk()?;
        walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        walk.push(head_commit.id())?;
        walk.hide(merge_base)?;
        walk.map(|entry| {
            let commit = self.repo.find_commit(entry?)?;
            let id = commit.id().to_string();
            Ok(CommitInfo {
                short_id: id[..8].to_owned(),
                id,
                title: commit.summary().unwrap_or("(no title)").to_owned(),
            })
        })
        .collect()
    }

    fn collect_diff(
        &self,
        diff: git2::Diff<'_>,
        files: &mut BTreeMap<String, ReviewFile>,
    ) -> Result<()> {
        diff.print(DiffFormat::Patch, |delta, _hunk, line| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(|path| path.to_str())
                .unwrap_or("binary file")
                .to_owned();
            let entry = files.entry(path.clone()).or_insert_with(|| ReviewFile {
                path,
                status: change_kind(delta.status()),
                additions: 0,
                deletions: 0,
                lines: Vec::new(),
            });
            let origin = line.origin();
            if matches!(origin, '+' | '-' | ' ' | 'H') {
                if origin == '+' {
                    entry.additions += 1;
                }
                if origin == '-' {
                    entry.deletions += 1;
                }
                entry.lines.push(DiffLine {
                    origin,
                    old_line: line.old_lineno(),
                    new_line: line.new_lineno(),
                    content: sanitize_diff_content(line.content()),
                });
            }
            true
        })?;
        Ok(())
    }
}

fn sanitize_diff_content(bytes: &[u8]) -> String {
    let source = String::from_utf8_lossy(bytes);
    let source = source.trim_end_matches(['\n', '\r']);
    let mut sanitized = String::with_capacity(source.len());
    for character in source.chars() {
        match character {
            '\t' => {
                for _ in 0..TAB_WIDTH {
                    sanitized.push(' ');
                }
            }
            character if character.is_ascii_control() => {
                let picture = if character == '\u{7f}' {
                    '\u{2421}'
                } else {
                    char::from_u32(0x2400 + character as u32).unwrap_or('�')
                };
                sanitized.push(picture);
            }
            character if character.is_control() => sanitized.push('�'),
            character => sanitized.push(character),
        }
    }
    sanitized
}

fn diff_options() -> DiffOptions {
    let mut options = DiffOptions::new();
    options
        .include_typechange(true)
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    options
}

fn change_kind(status: git2::Delta) -> ChangeKind {
    match status {
        git2::Delta::Added => ChangeKind::Added,
        git2::Delta::Deleted => ChangeKind::Deleted,
        git2::Delta::Modified => ChangeKind::Modified,
        git2::Delta::Renamed => ChangeKind::Renamed,
        git2::Delta::Typechange => ChangeKind::Typechange,
        _ => ChangeKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_diff_content;

    #[test]
    fn diff_content_expands_tabs_and_removes_line_endings() {
        assert_eq!(sanitize_diff_content(b"\tvalue\t\r\n"), "    value    ");
    }

    #[test]
    fn diff_content_makes_terminal_controls_visible() {
        assert_eq!(
            sanitize_diff_content(b"before\x1b[31mafter\n"),
            "before␛[31mafter"
        );
    }
}
