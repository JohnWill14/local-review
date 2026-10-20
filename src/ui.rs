use crate::{
    app::{App, Focus, Modal, RefTarget, VisualMode},
    git::{ChangeKind, DiffLine, RefKind},
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Widget, Wrap},
    Frame, Terminal,
};
use std::{
    collections::BTreeSet,
    io::{self, stdout},
    time::Duration,
};
use unicode_width::UnicodeWidthStr;

pub fn run(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    // Ratatui starts with an empty back buffer, but terminals may retain content in the
    // alternate screen. Clear the physical terminal before the first diff-based draw.
    terminal.clear()?;
    let result = event_loop(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;
        if app.should_quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(180))? {
            match event::read()? {
                Event::Key(key) => handle_key(app, key)?,
                Event::Resize(_, _) => terminal.clear()?,
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    if let Some(modal) = app.modal.take() {
        return handle_modal(app, modal, key);
    }
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Esc => {
            if !app.cancel_visual() {
                app.should_quit = true;
            }
        }
        KeyCode::Char('?') | KeyCode::Char('H') => app.modal = Some(Modal::Help { all: true }),
        KeyCode::Char('h') => app.modal = Some(Modal::Help { all: false }),
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Tree => Focus::Diff,
                Focus::Diff => Focus::Notes,
                Focus::Notes => Focus::Tree,
            }
        }
        KeyCode::BackTab => {
            app.focus = match app.focus {
                Focus::Tree => Focus::Notes,
                Focus::Diff => Focus::Tree,
                Focus::Notes => Focus::Diff,
            }
        }
        KeyCode::Char('j') | KeyCode::Down => match app.focus {
            Focus::Tree => app.move_file(1),
            Focus::Diff => app.move_diff(1),
            Focus::Notes => app.move_note(1),
        },
        KeyCode::Char('k') | KeyCode::Up => match app.focus {
            Focus::Tree => app.move_file(-1),
            Focus::Diff => app.move_diff(-1),
            Focus::Notes => app.move_note(-1),
        },
        KeyCode::Char('b') => app.open_ref_picker(RefTarget::Base),
        KeyCode::Char('l') => app.open_ref_picker(RefTarget::Head),
        KeyCode::Char('u') => {
            app.split = !app.split;
            app.status = if app.split {
                "Split diff"
            } else {
                "Unified diff"
            }
            .into();
        }
        KeyCode::Char('n') if app.focus != Focus::Diff => app.show_notes = !app.show_notes,
        KeyCode::Char('w') => {
            app.include_worktree = !app.include_worktree;
            app.refresh()?;
        }
        KeyCode::Char('r') => app.refresh()?,
        KeyCode::Char('g') => app.open_commits(),
        KeyCode::Char('f') if matches!(app.focus, Focus::Tree | Focus::Diff) => app.open_search(),
        KeyCode::Char('n') if app.focus == Focus::Diff => app.goto_match(1),
        KeyCode::Char('N') if app.focus == Focus::Diff => app.goto_match(-1),
        KeyCode::Enter if app.focus == Focus::Notes => app.open_note_editor(),
        KeyCode::Char('X') => app.open_export_notes(),
        KeyCode::Char('v') if app.focus == Focus::Diff => app.toggle_visual(VisualMode::Character),
        KeyCode::Char('V') if app.focus == Focus::Diff => app.toggle_visual(VisualMode::Line),
        KeyCode::Char('c') if app.focus == Focus::Diff => {
            app.modal = Some(Modal::Comment {
                value: String::new(),
            })
        }
        KeyCode::Char('t') => app.cycle_theme()?,
        _ => {}
    };
    Ok(())
}

fn handle_modal(app: &mut App, modal: Modal, key: KeyEvent) -> Result<()> {
    match modal {
        Modal::RefPicker {
            target,
            mut selected,
        } => {
            match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => return app.apply_ref(target, selected),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(app.refs.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                _ => {}
            }
            app.modal = Some(Modal::RefPicker { target, selected });
        }
        Modal::Comment { mut value } => match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => return app.add_note(value),
            KeyCode::Backspace => {
                value.pop();
                app.modal = Some(Modal::Comment { value });
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                value.push(ch);
                app.modal = Some(Modal::Comment { value });
            }
            _ => app.modal = Some(Modal::Comment { value }),
        },
        Modal::Commits { mut selected } => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(app.commits.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Char('y') => {
                    if let Some(commit) = app.commits.get(selected) {
                        match arboard::Clipboard::new()
                            .and_then(|mut clipboard| clipboard.set_text(commit.id.clone()))
                        {
                            Ok(()) => {
                                app.status =
                                    format!("Hash {} copied to the clipboard", commit.short_id)
                            }
                            Err(error) => app.status = format!("Could not copy hash: {error}"),
                        }
                    }
                }
                _ => {}
            }
            app.modal = Some(Modal::Commits { selected });
        }
        Modal::ExportNotes => match key.code {
            KeyCode::Esc | KeyCode::Char('n') => {}
            KeyCode::Enter | KeyCode::Char('y') => {
                let markdown = app.export_notes_markdown();
                match arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(markdown))
                {
                    Ok(()) => {
                        let count = app.clear_exported_notes()?;
                        app.status = format!("{count} note(s) copied to the clipboard and removed");
                    }
                    Err(error) => {
                        app.status = format!("Could not copy notes; nothing was removed: {error}");
                        app.modal = None;
                    }
                }
            }
            _ => app.modal = Some(Modal::ExportNotes),
        },
        Modal::Help { all } => match key.code {
            KeyCode::Esc
            | KeyCode::Char('h')
            | KeyCode::Char('H')
            | KeyCode::Char('?')
            | KeyCode::Enter => {}
            _ => app.modal = Some(Modal::Help { all }),
        },
        Modal::Search { scope, mut value } => match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => app.apply_search(scope, value),
            KeyCode::Backspace => {
                value.pop();
                app.modal = Some(Modal::Search { scope, value });
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                value.push(ch);
                app.modal = Some(Modal::Search { scope, value });
            }
            _ => app.modal = Some(Modal::Search { scope, value }),
        },
        Modal::EditNote { id, mut value } => match key.code {
            KeyCode::Esc => {}
            KeyCode::Enter => return app.update_note(id, value),
            KeyCode::Backspace => {
                value.pop();
                app.modal = Some(Modal::EditNote { id, value });
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                value.push(ch);
                app.modal = Some(Modal::EditNote { id, value });
            }
            _ => app.modal = Some(Modal::EditNote { id, value }),
        },
    }
    Ok(())
}

fn render(frame: &mut Frame, app: &App) {
    frame.render_widget(
        OpaqueFill(
            Style::default()
                .bg(app.theme.canvas)
                .fg(app.theme.foreground),
        ),
        frame.area(),
    );
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(if app.show_notes { 7 } else { 1 }),
            Constraint::Length(2),
        ])
        .split(frame.area());
    render_header(frame, areas[0], app);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(areas[1]);
    render_tree(frame, body[0], app);
    render_diff(frame, body[1], app);
    if app.show_notes {
        render_notes(frame, areas[2], app);
    }
    let footer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(areas[3]);
    frame.render_widget(
        Paragraph::new(app.status.as_str())
            .style(Style::default().fg(app.theme.muted).bg(app.theme.panel)),
        footer[0],
    );
    frame.render_widget(
        Paragraph::new(contextual_help(app)).style(
            Style::default()
                .fg(app.theme.foreground)
                .bg(app.theme.panel),
        ),
        footer[1],
    );
    if let Some(modal) = &app.modal {
        render_modal(frame, modal, app);
    }
}

fn contextual_help(app: &App) -> String {
    match app.modal {
        Some(Modal::RefPicker { .. }) => "  Up/k  Down/j navigate   Enter confirms   Esc cancels".into(),
        Some(Modal::Comment { .. }) => "  Type a comment   Enter saves   Esc discards".into(),
        Some(Modal::Commits { .. }) => "  Up/k  Down/j navigate   y copies hash   Esc closes".into(),
        Some(Modal::ExportNotes) => "  Enter/y exports and removes   Esc/n cancels".into(),
        Some(Modal::Help { .. }) => "  Esc, Enter, h, H, or ? closes help".into(),
        Some(Modal::Search { .. }) => "  Type to search   Enter confirms   Esc cancels".into(),
        Some(Modal::EditNote { .. }) => "  Enter saves   Esc discards".into(),
        None => match app.focus {
            Focus::Tree => "  Files: Up/k  Down/j select   Tab/Shift+Tab panels   b base   l head   g commits   X export notes   h help".into(),
            Focus::Diff => "  Diff: Up/k  Down/j move   v/V select lines   c comment   u split   X export notes   h help".into(),
            Focus::Notes => "  Notes: X exports to clipboard and removes   n hides panel   Tab/Shift+Tab panels   h help".into(),
        },
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let work = if app.include_worktree {
        " + working tree"
    } else {
        ""
    };
    let text = Line::from(vec![
        Span::styled(
            " ◇ Local Review ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", app.repo.display_name()),
            Style::default().fg(app.theme.foreground),
        ),
        Span::styled(
            format!("Base: {}  →  Head: {}{}", app.base, app.head, work),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled(
            format!(
                "   [{} · {}]",
                if app.split { "split" } else { "unified" },
                app.theme.name
            ),
            Style::default().fg(app.theme.accent),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::BOTTOM))
            .style(Style::default().bg(app.theme.canvas)),
        area,
    );
}

fn render_tree(frame: &mut Frame, area: Rect, app: &App) {
    render_panel_background(frame, area, app);
    let mut seen_directories = BTreeSet::new();
    let mut items = Vec::new();
    let mut selected_row = None;
    for index in app.visible_files() {
        let file = &app.comparison.files[index];
        let parts: Vec<_> = file.path.split('/').collect();
        for depth in 1..parts.len() {
            let directory = parts[..depth].join("/");
            if seen_directories.insert(directory) {
                items.push(ListItem::new(Line::from(Span::styled(
                    format!("{}▾ {}/", "  ".repeat(depth - 1), parts[depth - 1]),
                    Style::default().fg(app.theme.muted),
                ))));
            }
        }
        let depth = parts.len().saturating_sub(1);
        let name = parts.last().copied().unwrap_or(&file.path);
        let status = match file.status {
            ChangeKind::Added => "A",
            ChangeKind::Deleted => "D",
            ChangeKind::Modified => "M",
            ChangeKind::Renamed => "R",
            _ => "•",
        };
        let prefix = format!("{}{} ", "  ".repeat(depth), status);
        let row = items.len();
        if index == app.selected_file {
            selected_row = Some(row);
        }
        let style = if index == app.selected_file {
            Style::default()
                .bg(app.theme.selection)
                .fg(app.theme.foreground)
        } else {
            Style::default().fg(app.theme.foreground)
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, style.fg(status_color(file.status, app))),
            Span::styled(name.to_owned(), style),
            Span::styled(" +", style.fg(app.theme.added)),
            Span::styled(file.additions.to_string(), style.fg(app.theme.added)),
            Span::styled(" -", style.fg(app.theme.removed)),
            Span::styled(file.deletions.to_string(), style.fg(app.theme.removed)),
        ])));
    }
    let title = format!(
        " Files ({}){}{}",
        app.visible_files().len(),
        if app.file_query.is_empty() {
            ""
        } else {
            " · filter"
        },
        if app.focus == Focus::Tree {
            " · active"
        } else {
            ""
        }
    );
    let visible_rows = area.height.saturating_sub(2) as usize;
    let selected_row = selected_row.unwrap_or(0);
    let offset = selected_row.saturating_sub(visible_rows.saturating_sub(1));
    let mut state = ListState::default()
        .with_selected((selected_row < items.len()).then_some(selected_row))
        .with_offset(offset);
    frame.render_stateful_widget(
        List::new(items)
            .style(Style::default().bg(app.theme.panel))
            .block(panel_block(app, Focus::Tree, title)),
        area,
        &mut state,
    );
}

fn render_diff(frame: &mut Frame, area: Rect, app: &App) {
    let Some(file) = app.comparison.files.get(app.selected_file) else {
        render_panel_background(frame, area, app);
        frame.render_widget(
            Paragraph::new("No changes between the selected references.")
                .style(Style::default().bg(app.theme.panel))
                .block(panel_block(app, Focus::Diff, " Diff ".into())),
            area,
        );
        return;
    };
    let title = format!(
        " {}{}",
        file.path,
        if app.focus == Focus::Diff {
            " · active"
        } else {
            ""
        }
    );
    if app.split && area.width > 90 {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        render_panel_background(frame, panes[0], app);
        let left_block = panel_block(app, Focus::Diff, format!("{} (before)", title));
        let left_width = left_block.inner(panes[0]).width;
        frame.render_widget(
            diff_paragraph(
                file.lines
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| line.origin != '+'),
                app,
                true,
                left_width,
            )
            .block(left_block),
            panes[0],
        );
        render_panel_background(frame, panes[1], app);
        let right_block = panel_block(app, Focus::Diff, format!("{} (after)", title));
        let right_width = right_block.inner(panes[1]).width;
        frame.render_widget(
            diff_paragraph(
                file.lines
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| line.origin != '-'),
                app,
                false,
                right_width,
            )
            .block(right_block),
            panes[1],
        );
    } else {
        render_panel_background(frame, area, app);
        let block = panel_block(app, Focus::Diff, title);
        let width = block.inner(area).width;
        frame.render_widget(
            diff_paragraph(file.lines.iter().enumerate(), app, false, width).block(block),
            area,
        );
    }
}

fn diff_paragraph<'a>(
    lines: impl Iterator<Item = (usize, &'a DiffLine)>,
    app: &App,
    old: bool,
    width: u16,
) -> Paragraph<'a> {
    let text: Vec<Line> = lines
        .map(|(index, line)| {
            let number = if old {
                line.old_line
            } else {
                line.new_line.or(line.old_line)
            }
            .map(|number| format!("{number:>4} "))
            .unwrap_or_else(|| "     ".into());
            let color = match line.origin {
                '+' => app.theme.added,
                '-' => app.theme.removed,
                'H' => app.theme.accent,
                _ => app.theme.foreground,
            };
            let content = if line.origin == 'H' {
                line.content.clone()
            } else {
                format!("{}{}", line.origin, line.content)
            };
            let selected = app.focus == Focus::Diff
                && app.is_line_selected(index)
                && matches!(line.origin, '+' | '-' | ' ');
            let matched = app.diff_matches.contains(&index);
            let selection_style = if selected || matched {
                Style::default().bg(app.theme.selection)
            } else {
                Style::default().bg(app.theme.panel)
            };
            let prefix = format!("{number}{content}");
            let padding = " "
                .repeat((width as usize).saturating_sub(UnicodeWidthStr::width(prefix.as_str())));
            Line::from(vec![
                Span::styled(number, selection_style.fg(app.theme.muted)),
                Span::styled(content, selection_style.fg(color)),
                Span::styled(padding, selection_style),
            ])
        })
        .collect();
    Paragraph::new(text)
        .style(Style::default().bg(app.theme.panel))
        .wrap(Wrap { trim: false })
}

fn render_notes(frame: &mut Frame, area: Rect, app: &App) {
    render_panel_background(frame, area, app);
    let items: Vec<ListItem> = app
        .note_list
        .iter()
        .take(4)
        .enumerate()
        .map(|(index, note)| {
            let style = if index == app.selected_note {
                Style::default()
                    .bg(app.theme.selection)
                    .fg(app.theme.foreground)
            } else {
                Style::default().fg(app.theme.foreground)
            };
            ListItem::new(Line::styled(
                format!(
                    "{}:{}  {}",
                    note.path,
                    note.start_line
                        .map(|line| if note.end_line.unwrap_or(line) == line {
                            line.to_string()
                        } else {
                            format!("{line}–{}", note.end_line.unwrap_or(line))
                        })
                        .unwrap_or_else(|| "-".into()),
                    note.body
                ),
                style,
            ))
        })
        .collect();
    let title = format!(
        " Notes ({}){}",
        app.note_list.len(),
        if app.focus == Focus::Notes {
            " · active"
        } else {
            ""
        }
    );
    frame.render_widget(
        List::new(items)
            .style(Style::default().bg(app.theme.panel))
            .block(panel_block(app, Focus::Notes, title)),
        area,
    );
}

fn render_modal(frame: &mut Frame, modal: &Modal, app: &App) {
    let area = centered_rect(68, 60, frame.area());
    frame.render_widget(OpaqueFill(Style::default().bg(app.theme.modal)), area);
    match modal {
        Modal::RefPicker { target, selected } => {
            let title = if *target == RefTarget::Base {
                " Choose base "
            } else {
                " Choose head "
            };
            let items: Vec<ListItem> = app
                .refs
                .iter()
                .enumerate()
                .map(|(i, reference)| {
                    let marker = if i == *selected { "▶ " } else { "  " };
                    let kind = match reference.kind {
                        RefKind::Local => "branch",
                        RefKind::Remote => "remote",
                        RefKind::Tag => "tag",
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(marker, Style::default().fg(app.theme.accent)),
                        Span::raw(&reference.name),
                        Span::styled(format!("  {kind}"), Style::default().fg(app.theme.muted)),
                    ]))
                })
                .collect();
            frame.render_widget(
                List::new(items).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .style(Style::default().bg(app.theme.modal)),
                ),
                area,
            );
        }
        Modal::Comment { value } => {
            let path = app.selected_path().unwrap_or("file");
            let range = app
                .selected_line_range()
                .map(|(start, end)| {
                    if start == end {
                        start.to_string()
                    } else {
                        format!("{start}–{end}")
                    }
                })
                .unwrap_or_else(|| "?".into());
            frame.render_widget(
                Paragraph::new(format!("{value}▌"))
                    .wrap(Wrap { trim: false })
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(
                                " Comment on {path}:{range} — Enter saves / Esc cancels "
                            ))
                            .style(Style::default().bg(app.theme.modal)),
                    ),
                area,
            );
        }
        Modal::Commits { selected } => {
            let items: Vec<ListItem> = app
                .commits
                .iter()
                .enumerate()
                .map(|(index, commit)| {
                    let style = if index == *selected {
                        Style::default()
                            .bg(app.theme.selection)
                            .fg(app.theme.foreground)
                    } else {
                        Style::default().fg(app.theme.foreground)
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{}  ", commit.short_id),
                            Style::default().fg(app.theme.accent),
                        ),
                        Span::styled(&commit.title, style),
                    ]))
                })
                .collect();
            frame.render_widget(
                List::new(items).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(
                            " Commits ({}): j/k navigate · y copies hash · Esc closes ",
                            app.commits.len()
                        ))
                        .style(Style::default().bg(app.theme.modal)),
                ),
                area,
            );
        }
        Modal::ExportNotes => {
            frame.render_widget(
                Paragraph::new(format!("Export {} note(s) as Markdown to the clipboard and remove them from this repository?\n\nEnter or y confirms · Esc or n cancels", app.note_list.len()))
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL).title(" Export and clear notes ").style(Style::default().bg(app.theme.modal))),
                area,
            );
        }
        Modal::Help { all } => {
            let content = if *all {
                "Navigation\n  Tab / Shift+Tab  panel right / left\n  j/k or arrows     navigate\n\nReview\n  b                  choose base\n  l                  choose head\n  g                  list commits\n  u                  toggle unified/split diff\n  w                  include working tree\n\nDiff\n  v / V              start visual selection\n  c                  comment selected line(s)\n\nNotes\n  X                  copy notes and snippets to clipboard; then clear\n  n                  show/hide notes\n\nInterface\n  t                  switch theme\n  r                  refresh\n  q                  quit\n\nEsc, Enter, h, H, or ? closes this help"
            } else {
                match app.focus {
                    Focus::Tree => "Files\n  j/k or arrows  select file\n  Tab            go to diff\n  b              choose base\n  l              choose head\n  g              list commits\n  t              switch theme\n  X              export notes to clipboard and clear\n\nEsc, Enter, or h closes this help",
                    Focus::Diff => "Diff\n  j/k or arrows  move cursor\n  v / V          start visual selection\n  c              comment selection\n  u              toggle unified/split\n  t              switch theme\n  Tab            go to notes\n  X              export notes to clipboard and clear\n\nEsc, Enter, or h closes this help",
                    Focus::Notes => "Notes\n  X              export to clipboard and confirm clearing\n  n              hide panel\n  t              switch theme\n  Tab            go to files\n\nEsc, Enter, or h closes this help",
                }
            };
            frame.render_widget(
                Paragraph::new(content).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(if *all {
                            " Shortcuts "
                        } else {
                            " Panel shortcuts "
                        })
                        .style(Style::default().bg(app.theme.modal)),
                ),
                area,
            );
        }
        Modal::Search { scope, value } => {
            let label = match scope {
                Focus::Tree => "Search files",
                Focus::Diff => "Search diff",
                Focus::Notes => "Search",
            };
            frame.render_widget(
                Paragraph::new(format!("{value}▌")).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {label} — Enter confirms / Esc cancels "))
                        .style(Style::default().bg(app.theme.modal)),
                ),
                area,
            );
        }
        Modal::EditNote { value, .. } => {
            frame.render_widget(
                Paragraph::new(format!("{value}▌"))
                    .wrap(Wrap { trim: false })
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Edit comment — Enter saves / Esc cancels ")
                            .style(Style::default().bg(app.theme.modal)),
                    ),
                area,
            );
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

struct OpaqueFill(Style);

impl Widget for OpaqueFill {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = &mut buffer[(x, y)];
                cell.reset();
                cell.set_style(self.0);
            }
        }
    }
}

fn render_panel_background(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(
        OpaqueFill(
            Style::default()
                .bg(app.theme.panel)
                .fg(app.theme.foreground),
        ),
        area,
    );
}
fn status_color(status: ChangeKind, app: &App) -> ratatui::style::Color {
    match status {
        ChangeKind::Added => app.theme.added,
        ChangeKind::Deleted => app.theme.removed,
        _ => app.theme.accent,
    }
}

#[cfg(test)]
mod tests {
    use super::OpaqueFill;
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Style},
        widgets::Widget,
    };

    #[test]
    fn opaque_fill_erases_previous_symbols_and_applies_its_style() {
        let mut buffer = Buffer::with_lines(["abcdef", "ghijkl", "mnopqr"]);
        let style = Style::default().bg(Color::Blue).fg(Color::White);
        buffer[(2, 1)].set_symbol("表").set_skip(true);

        OpaqueFill(style).render(Rect::new(1, 1, 4, 1), &mut buffer);

        for x in 1..5 {
            assert_eq!(buffer[(x, 1)].symbol(), " ");
            assert_eq!(buffer[(x, 1)].fg, Color::White);
            assert_eq!(buffer[(x, 1)].bg, Color::Blue);
            assert!(!buffer[(x, 1)].skip);
        }
        assert_eq!(buffer[(0, 1)].symbol(), "g");
        assert_eq!(buffer[(5, 1)].symbol(), "l");
    }
}

fn panel_block(app: &App, focus: Focus, title: String) -> Block<'static> {
    let active = app.focus == focus;
    let color = if active {
        app.theme.accent
    } else {
        app.theme.muted
    };
    Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(app.theme.panel))
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            title,
            Style::default().fg(color).add_modifier(if active {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ))
}
