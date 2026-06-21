//! Input handling: translating key events into application commands.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::App;

/// Lines moved per page-scroll key (Ctrl-d / Ctrl-u). Picked as a fixed count
/// rather than half the visible height so input handling stays decoupled from
/// the rendered pane size.
const PREVIEW_SCROLL_PAGE: usize = 10;

/// Apply a single key press to the application state. Key releases and repeats
/// are ignored so each physical press triggers exactly one command.
pub fn handle_key(app: &mut App, key_event: KeyEvent) {
    if key_event.kind != KeyEventKind::Press {
        return;
    }

    match key_event.code {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('g') | KeyCode::Home => app.jump_first(),
        KeyCode::Char('G') | KeyCode::End => app.jump_last(),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => app.enter_selected(),
        KeyCode::Char('h') | KeyCode::Left => app.enter_parent(),
        KeyCode::Char('J') => app.scroll_preview_down(1),
        KeyCode::Char('K') => app.scroll_preview_up(1),
        KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_preview_down(PREVIEW_SCROLL_PAGE);
        }
        KeyCode::Char('u') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_preview_up(PREVIEW_SCROLL_PAGE);
        }
        _ => {}
    }
}
