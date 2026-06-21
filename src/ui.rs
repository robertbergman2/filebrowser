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
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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

    let title = Paragraph::new(sanitize_for_terminal(
        &app.current_dir().display().to_string(),
        false,
    ))
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
                Span::raw(sanitize_for_terminal(&entry.name, false)),
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
        return (
            sanitize_for_terminal(&entry.name, false),
            vec![Line::from("<directory>")],
        );
    }

    let path = app.current_dir().join(&entry.name);
    let content =
        filesystem::read_preview(&path).unwrap_or_else(|| "<binary or unreadable>".to_string());
    let lines = wrap_text(&sanitize_for_terminal(&content, true), width)
        .into_iter()
        .map(Line::from)
        .collect();
    (sanitize_for_terminal(&entry.name, false), lines)
}

/// Render `content` safely by replacing terminal control characters with their
/// visible hexadecimal escapes. Newlines are optionally retained as preview
/// line delimiters; every other control character is escaped.
fn sanitize_for_terminal(content: &str, allow_newlines: bool) -> String {
    let mut sanitized = String::with_capacity(content.len());
    for character in content.chars() {
        if character == '\n' && allow_newlines {
            sanitized.push(character);
        } else if character.is_control() {
            use std::fmt::Write;

            let _ = write!(sanitized, "\\x{:02X}", character as u32);
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

/// Greedy word-wrap `content` to `width` terminal columns, returning one
/// string per visual row. It preserves all whitespace and keeps Unicode
/// grapheme clusters intact, while splitting oversized words at grapheme
/// boundaries when necessary.
fn wrap_text(content: &str, width: usize) -> Vec<String> {
    let mut output = Vec::new();
    for logical_line in content.split('\n') {
        let line = logical_line.strip_suffix('\r').unwrap_or(logical_line);
        if width == 0 || UnicodeWidthStr::width(line) <= width {
            output.push(line.to_string());
            continue;
        }

        let mut current = Vec::new();
        let mut current_width = 0;
        for grapheme in UnicodeSegmentation::graphemes(line, true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);

            while current_width > 0 && current_width + grapheme_width > width {
                let break_at = current
                    .iter()
                    .rposition(|segment: &&str| segment.chars().all(char::is_whitespace));

                if let Some(break_at) = break_at {
                    let next = current.split_off(break_at + 1);
                    output.push(current.concat());
                    current = next;
                    current_width = current
                        .iter()
                        .map(|segment| UnicodeWidthStr::width(*segment))
                        .sum();
                } else {
                    output.push(current.concat());
                    current.clear();
                    current_width = 0;
                }
            }

            current.push(grapheme);
            current_width += grapheme_width;
        }

        output.push(current.concat());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{sanitize_for_terminal, wrap_text};

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
        assert_eq!(wrapped.concat(), "supercalifragilisticexpialidocious");
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

    #[test]
    fn wide_characters_wrap_at_display_width() {
        assert_eq!(wrap_text("界界界", 4), vec!["界界", "界"]);
    }

    #[test]
    fn grapheme_clusters_are_not_split() {
        assert_eq!(
            wrap_text("e\u{301}e\u{301}", 1),
            vec!["e\u{301}", "e\u{301}"]
        );
    }

    #[test]
    fn terminal_controls_are_rendered_as_visible_text() {
        assert_eq!(
            sanitize_for_terminal("name\u{1b}]52;c;payload\u{7}", false),
            "name\\x1B]52;c;payload\\x07"
        );
        assert_eq!(sanitize_for_terminal("a\nb\t", true), "a\nb\\x09");
    }
}
