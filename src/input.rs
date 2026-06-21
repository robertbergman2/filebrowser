//! Input handling: translating key events into application commands.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::app::App;

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
        _ => {}
    }
}
