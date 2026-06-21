//! Rendering: turning application state into ratatui widgets.
//!
//! This module only reads application state (plus the scroll adjustment that
//! depends on the rendered area); it never decides navigation.

use std::time::SystemTime;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::filesystem;

/// Draw the full UI: path header, file list, preview pane, and key hints.
pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);

    let title = Paragraph::new(app.current_dir().display().to_string())
        .block(Block::default().borders(Borders::ALL).title("Path"));
    frame.render_widget(title, rows[0]);

    render_file_list(frame, app, body[0]);
    render_preview(frame, app, body[1]);

    let status = Paragraph::new(status_text(app))
        .block(Block::default().borders(Borders::ALL).title("Keys"));
    frame.render_widget(status, rows[2]);
}

fn status_text(app: &App) -> String {
    let selected_details = app.selected_entry().map_or_else(
        || "no selection".to_string(),
        |entry| {
            let modified = entry
                .modified
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            format!("selected: {} bytes, modified: {modified}s", entry.size)
        },
    );

    format!(
        "j/k or arrows: move | h/l or arrows: parent/open | g/G: first/last | q or Ctrl+C: quit | {} items | {selected_details}",
        app.entries().len()
    )
}

fn render_file_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let visible_height = area.height.saturating_sub(2) as usize;
    app.clamp_scroll(visible_height);

    let scroll_offset = app.scroll_offset();
    let selected = app.selected();

    let items = app
        .entries()
        .iter()
        .skip(scroll_offset)
        .take(visible_height)
        .enumerate()
        .map(|(visible_index, entry)| {
            let prefix = if entry.is_dir { "/" } else { " " };
            let style = if scroll_offset + visible_index == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::raw(" "),
                Span::raw(entry.name.as_str()),
            ]))
            .style(style)
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Files"));
    frame.render_widget(list, area);
}

fn render_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let (title, content) = preview_content(app);
    let preview = Paragraph::new(content)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(preview, area);
}

fn preview_content(app: &App) -> (String, String) {
    let Some(entry) = app.selected_entry() else {
        return ("Preview".to_string(), String::new());
    };

    if entry.is_dir {
        return (entry.name.clone(), "<directory>".to_string());
    }

    let path = app.current_dir().join(&entry.name);
    let content =
        filesystem::read_preview(&path).unwrap_or_else(|| "<binary or unreadable>".to_string());
    (entry.name.clone(), content)
}
