use crate::{
    config::Config,
    git::{CommitInfo, Comparison, GitRef, RepositoryModel},
    notes::{format_markdown, Note, NoteStore},
    theme::Theme,
};
use anyhow::Result;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Diff,
    Notes,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RefTarget {
    Base,
    Head,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Character,
    Line,
}
pub enum Modal {
    RefPicker { target: RefTarget, selected: usize },
    Comment { value: String },
    Commits { selected: usize },
    ExportNotes,
    Help { all: bool },
    Search { scope: Focus, value: String },
    EditNote { id: i64, value: String },
}

pub struct App {
    pub repo: RepositoryModel,
    pub notes: NoteStore,
    pub base: String,
    pub head: String,
    pub include_worktree: bool,
    pub refs: Vec<GitRef>,
    pub comparison: Comparison,
    pub commits: Vec<CommitInfo>,
    pub note_list: Vec<Note>,
    pub selected_file: usize,
    pub selected_note: usize,
    pub file_query: String,
    pub diff_query: String,
    pub diff_matches: Vec<usize>,
    pub diff_match: usize,
    pub diff_cursor: usize,
    pub visual_mode: Option<VisualMode>,
    pub selection_anchor: Option<usize>,
    pub focus: Focus,
    pub split: bool,
    pub show_notes: bool,
    pub theme: Theme,
    pub modal: Option<Modal>,
    pub status: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        repo: RepositoryModel,
        notes: NoteStore,
        base: String,
        head: String,
        include_worktree: bool,
        theme: String,
    ) -> Result<Self> {
        let refs = repo.refs()?;
        let comparison = repo.comparison(&base, &head, include_worktree)?;
        let commits = repo.commits_between(&base, &head)?;
        let note_list = notes.list(&repo.root.to_string_lossy())?;
        Ok(Self {
            repo,
            notes,
            base,
            head,
            include_worktree,
            refs,
            comparison,
            commits,
            note_list,
            selected_file: 0,
            selected_note: 0,
            file_query: String::new(),
            diff_query: String::new(),
            diff_matches: Vec::new(),
            diff_match: 0,
            diff_cursor: 0,
            visual_mode: None,
            selection_anchor: None,
            focus: Focus::Tree,
            split: true,
            show_notes: true,
            theme: Theme::named(&theme),
            modal: None,
            status: "Ready — press ? for shortcuts".into(),
            should_quit: false,
        })
    }
    pub fn refresh(&mut self) -> Result<()> {
        self.comparison = self
            .repo
            .comparison(&self.base, &self.head, self.include_worktree)?;
        self.commits = self.repo.commits_between(&self.base, &self.head)?;
        self.selected_file = self
            .selected_file
            .min(self.comparison.files.len().saturating_sub(1));
        self.reset_diff_cursor();
        self.note_list = self.notes.list(&self.repo.root.to_string_lossy())?;
        self.status = format!("{} changed file(s)", self.comparison.files.len());
        Ok(())
    }
    pub fn open_commits(&mut self) {
        self.modal = Some(Modal::Commits { selected: 0 });
    }
    pub fn cycle_theme(&mut self) -> Result<()> {
        self.theme = self.theme.next();
        Config::save_theme(&self.theme.key)?;
        self.status = format!("Theme changed to {}", self.theme.name);
        Ok(())
    }
    pub fn open_export_notes(&mut self) {
        if self.note_list.is_empty() {
            self.status = "There are no notes to export".into();
        } else {
            self.modal = Some(Modal::ExportNotes);
        }
    }
    pub fn export_notes_markdown(&self) -> String {
        format_markdown(&self.note_list)
    }
    pub fn clear_exported_notes(&mut self) -> Result<usize> {
        let count = self.notes.clear(&self.repo.root.to_string_lossy())?;
        self.note_list.clear();
        self.modal = None;
        Ok(count)
    }
    pub fn selected_path(&self) -> Option<&str> {
        self.comparison
            .files
            .get(self.selected_file)
            .map(|file| file.path.as_str())
    }
    pub fn move_file(&mut self, amount: isize) {
        let visible = self.visible_files();
        let current = visible
            .iter()
            .position(|index| *index == self.selected_file)
            .unwrap_or(0);
        if let Some(index) = visible.get(move_index(current, visible.len(), amount)) {
            self.selected_file = *index;
        }
        self.reset_diff_cursor();
    }
    pub fn move_note(&mut self, amount: isize) {
        self.selected_note = move_index(self.selected_note, self.note_list.len(), amount);
    }
    pub fn visible_files(&self) -> Vec<usize> {
        let query = self.file_query.to_lowercase();
        self.comparison
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                (query.is_empty() || file.path.to_lowercase().contains(&query)).then_some(index)
            })
            .collect()
    }
    pub fn open_search(&mut self) {
        self.modal = Some(Modal::Search {
            scope: self.focus,
            value: String::new(),
        });
    }
    pub fn apply_search(&mut self, scope: Focus, value: String) {
        match scope {
            Focus::Tree => {
                self.file_query = value;
                if let Some(index) = self.visible_files().first() {
                    self.selected_file = *index;
                    self.reset_diff_cursor();
                }
            }
            Focus::Diff => {
                self.diff_query = value;
                self.refresh_diff_matches();
                self.goto_match(0);
            }
            Focus::Notes => {}
        }
        self.modal = None;
    }
    pub fn refresh_diff_matches(&mut self) {
        let query = self.diff_query.to_lowercase();
        self.diff_matches = self
            .comparison
            .files
            .get(self.selected_file)
            .map(|file| {
                file.lines
                    .iter()
                    .enumerate()
                    .filter_map(|(i, line)| {
                        (!query.is_empty()
                            && is_selectable(line.origin)
                            && line.content.to_lowercase().contains(&query))
                        .then_some(i)
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.diff_match = 0;
    }
    pub fn goto_match(&mut self, amount: isize) {
        if self.diff_matches.is_empty() {
            return;
        }
        self.diff_match = move_index(self.diff_match, self.diff_matches.len(), amount);
        self.diff_cursor = self.diff_matches[self.diff_match];
    }
    pub fn move_diff(&mut self, amount: isize) {
        let Some(file) = self.comparison.files.get(self.selected_file) else {
            return;
        };
        let selectable: Vec<usize> = file
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| is_selectable(line.origin).then_some(index))
            .collect();
        let current = selectable
            .iter()
            .position(|index| *index == self.diff_cursor)
            .unwrap_or(0);
        let next = move_index(current, selectable.len(), amount);
        if let Some(index) = selectable.get(next) {
            self.diff_cursor = *index;
        }
    }
    pub fn toggle_visual(&mut self, mode: VisualMode) {
        if self.visual_mode.is_some() {
            self.visual_mode = None;
            self.selection_anchor = None;
            self.status = "Selection cancelled".into();
        } else {
            self.visual_mode = Some(mode);
            self.selection_anchor = Some(self.diff_cursor);
            self.status =
                "Visual selection active — move with j/k, c to comment, Esc to cancel".into();
        }
    }
    pub fn cancel_visual(&mut self) -> bool {
        if self.visual_mode.take().is_some() {
            self.selection_anchor = None;
            self.status = "Selection cancelled".into();
            true
        } else {
            false
        }
    }
    pub fn is_line_selected(&self, index: usize) -> bool {
        let Some(anchor) = self.selection_anchor else {
            return index == self.diff_cursor && self.focus == Focus::Diff;
        };
        let (start, end) = selection_bounds(anchor, self.diff_cursor);
        (start..=end).contains(&index)
    }
    pub fn selected_line_range(&self) -> Option<(u32, u32)> {
        let file = self.comparison.files.get(self.selected_file)?;
        let anchor = self.selection_anchor.unwrap_or(self.diff_cursor);
        let (start, end) = selection_bounds(anchor, self.diff_cursor);
        let mut numbers = file.lines[start..=end]
            .iter()
            .filter(|line| is_selectable(line.origin))
            .filter_map(|line| line.new_line.or(line.old_line));
        let first = numbers.next()?;
        let (mut min, mut max) = (first, first);
        for number in numbers {
            min = min.min(number);
            max = max.max(number);
        }
        Some((min, max))
    }
    pub fn open_ref_picker(&mut self, target: RefTarget) {
        let current = if target == RefTarget::Base {
            &self.base
        } else {
            &self.head
        };
        let selected = self
            .refs
            .iter()
            .position(|item| &item.name == current)
            .unwrap_or(0);
        self.modal = Some(Modal::RefPicker { target, selected });
    }
    pub fn apply_ref(&mut self, target: RefTarget, index: usize) -> Result<()> {
        let name = self
            .refs
            .get(index)
            .map(|item| item.name.clone())
            .unwrap_or_default();
        if target == RefTarget::Base {
            self.base = name;
        } else {
            self.head = name;
        }
        self.modal = None;
        self.refresh()
    }
    pub fn add_note(&mut self, body: String) -> Result<()> {
        let Some(path) = self.selected_path().map(str::to_owned) else {
            return Ok(());
        };
        let Some((start_line, end_line)) = self.selected_line_range() else {
            self.status = "Select a code line before adding a comment".into();
            self.modal = None;
            return Ok(());
        };
        let snippet = self.selected_diff_snippet();
        let body = single_line_comment(&body);
        if body.is_empty() {
            self.modal = None;
            return Ok(());
        }
        self.notes.add(
            &self.repo.root.to_string_lossy(),
            &path,
            start_line,
            end_line,
            &snippet,
            &body,
        )?;
        self.modal = None;
        self.visual_mode = None;
        self.selection_anchor = None;
        self.note_list = self.notes.list(&self.repo.root.to_string_lossy())?;
        self.status = "Comment saved locally".into();
        Ok(())
    }
    pub fn open_note_editor(&mut self) {
        if let Some(note) = self.note_list.get(self.selected_note) {
            self.modal = Some(Modal::EditNote {
                id: note.id,
                value: single_line_comment(&note.body),
            });
        }
    }
    pub fn update_note(&mut self, id: i64, body: String) -> Result<()> {
        let body = single_line_comment(&body);
        if body.is_empty() {
            self.modal = None;
            return Ok(());
        }
        self.notes.update(id, &body)?;
        self.note_list = self.notes.list(&self.repo.root.to_string_lossy())?;
        self.modal = None;
        self.status = "Comment updated".into();
        Ok(())
    }
    fn selected_diff_snippet(&self) -> String {
        let Some(file) = self.comparison.files.get(self.selected_file) else {
            return String::new();
        };
        let anchor = self.selection_anchor.unwrap_or(self.diff_cursor);
        let (start, end) = selection_bounds(anchor, self.diff_cursor);
        file.lines[start..=end]
            .iter()
            .filter(|line| is_selectable(line.origin))
            .map(|line| format!("{}{}", line.origin, line.content))
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn reset_diff_cursor(&mut self) {
        self.visual_mode = None;
        self.selection_anchor = None;
        self.diff_cursor = self
            .comparison
            .files
            .get(self.selected_file)
            .and_then(|file| {
                file.lines
                    .iter()
                    .position(|line| is_selectable(line.origin))
            })
            .unwrap_or(0);
    }
}

fn is_selectable(origin: char) -> bool {
    matches!(origin, '+' | '-' | ' ')
}

fn move_index(current: usize, len: usize, amount: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + amount).clamp(0, len.saturating_sub(1) as isize) as usize
}

fn selection_bounds(anchor: usize, cursor: usize) -> (usize, usize) {
    if anchor <= cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    }
}

fn single_line_comment(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{move_index, selection_bounds, single_line_comment};

    #[test]
    fn selection_expands_in_either_direction() {
        assert_eq!(selection_bounds(3, 8), (3, 8));
        assert_eq!(selection_bounds(8, 3), (3, 8));
    }

    #[test]
    fn navigation_stops_at_collection_boundaries() {
        assert_eq!(move_index(0, 4, -1), 0);
        assert_eq!(move_index(3, 4, 1), 3);
        assert_eq!(move_index(3, 4, 100), 3);
        assert_eq!(move_index(0, 4, -100), 0);
        assert_eq!(move_index(0, 0, 1), 0);
    }

    #[test]
    fn comments_are_normalized_to_one_line() {
        assert_eq!(
            single_line_comment("  one\nnote\r\nlocal  "),
            "one note local"
        );
    }
}
