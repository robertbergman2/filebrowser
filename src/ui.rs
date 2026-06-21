//! Rendering: turning application state into ratatui widgets.
//!
//! This module only reads application state (plus the scroll adjustment that
//! depends on the rendered area); it never decides navigation.

use std::time::SystemTime;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
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
        "j/k or arrows: move | h/l or arrows: parent/open | g/G: first/last | J/K or Ctrl-d/u: scroll preview | q or Ctrl+C: quit | {} items | {selected_details}",
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

fn render_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    // Clear the area first: ratatui's Block only patches cell style, not the
    // symbol, so a shorter preview (e.g. "<directory>" after a long file)
    // would leave the previous frame's characters in the rows below.
    frame.render_widget(Clear, area);

    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    let inner_width = inner.width as usize;
    let visible_height = inner.height as usize;

    let (title, lines) = preview_lines(app, inner_width);
    app.clamp_preview_scroll(lines.len(), visible_height);
    let scroll = app.preview_scroll();

    let text = Text::from(lines);
    let scroll_y = u16::try_from(scroll).unwrap_or(u16::MAX);
    let preview = Paragraph::new(text)
        .block(block.title(title))
        .scroll((scroll_y, 0));

    frame.render_widget(preview, area);
}

fn preview_lines(app: &App, width: usize) -> (String, Vec<Line<'static>>) {
    let Some(entry) = app.selected_entry() else {
        return ("Preview".to_string(), Vec::new());
    };

    if entry.is_dir {
        return (entry.name.clone(), vec![Line::from("<directory>")]);
    }

    let path = app.current_dir().join(&entry.name);
    let content =
        filesystem::read_preview(&path).unwrap_or_else(|| "<binary or unreadable>".to_string());
    let lines = wrap_text(&content, width)
        .into_iter()
        .map(Line::from)
        .collect();
    (entry.name.clone(), lines)
}

/// Greedy word-wrap `content` to `width` columns, returning one string per
/// visual row. Leading whitespace on each input line is preserved so source
/// indentation survives; words longer than `width` are hard-broken. Width is
/// measured in `char` count (a close-enough proxy for display columns for the
/// ASCII-dominated content the preview is built for).
fn wrap_text(content: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for logical_line in content.split('\n') {
        let line = logical_line.strip_suffix('\r').unwrap_or(logical_line);
        if width == 0 || line.chars().count() <= width {
            out.push(line.to_string());
            continue;
        }

        let mut cur = String::new();
        let mut cur_len = 0usize;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            if cur_len == width {
                out.push(cur.clone());
                cur.clear();
                cur_len = 0;
            }
            cur.push(chars[i]);
            cur_len += 1;
            i += 1;
        }

        while i < chars.len() {
            let word_start = i;
            while i < chars.len() && chars[i] != ' ' && chars[i] != '\t' {
                i += 1;
            }
            let word: String = chars[word_start..i].iter().collect();
            let word_len = word.chars().count();

            let ws_start = i;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
            let ws: String = chars[ws_start..i].iter().collect();
            let ws_len = ws.chars().count();

            if word_len > width {
                if cur_len > 0 {
                    out.push(cur.clone());
                    cur.clear();
                    cur_len = 0;
                }
                for c in word.chars() {
                    if cur_len == width {
                        out.push(cur.clone());
                        cur.clear();
                        cur_len = 0;
                    }
                    cur.push(c);
                    cur_len += 1;
                }
                if cur_len + ws_len <= width {
                    cur.push_str(&ws);
                    cur_len += ws_len;
                }
            } else if cur_len + word_len <= width {
                cur.push_str(&word);
                cur_len += word_len;
                if cur_len + ws_len <= width {
                    cur.push_str(&ws);
                    cur_len += ws_len;
                }
            } else {
                out.push(cur.clone());
                cur.clear();
                cur.push_str(&word);
                cur_len = word_len;
                if cur_len + ws_len <= width {
                    cur.push_str(&ws);
                    cur_len += ws_len;
                }
            }
        }

        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wrap_text;

    #[test]
    fn short_line_passes_through_unchanged() {
        assert_eq!(wrap_text("hello", 10), vec!["hello"]);
    }

    #[test]
    fn blank_input_yields_one_empty_row() {
        assert_eq!(wrap_text("", 10), vec![""]);
    }

    #[test]
    fn leading_whitespace_is_preserved_across_wrap() {
        let input = "    fn main() { println!(\"hi\"); }";
        let wrapped = wrap_text(input, 16);
        assert_eq!(wrapped[0], "    fn main() { ");
        assert!(
            wrapped[0].starts_with("    "),
            "indentation must survive on the first wrapped row"
        );
    }

    #[test]
    fn long_word_is_hard_broken() {
        let wrapped = wrap_text("supercalifragilisticexpialidocious", 10);
        assert_eq!(wrapped.len(), 4);
        assert_eq!(wrapped.iter().map(|s| s.chars().count()).sum::<usize>(), 34);
    }

    #[test]
    fn crlf_lines_are_stripped_of_carriage_return() {
        let wrapped = wrap_text("a\r\nb", 10);
        assert_eq!(wrapped, vec!["a", "b"]);
    }

    #[test]
    fn empty_line_in_input_produces_empty_row() {
        assert_eq!(wrap_text("a\n\nb", 10), vec!["a", "", "b"]);
    }

    #[test]
    fn width_zero_falls_back_to_no_wrap() {
        assert_eq!(wrap_text("hello world", 0), vec!["hello world"]);
    }
}
