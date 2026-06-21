//! A minimal terminal file browser.
//!
//! `main.rs` is the composition root: it wires the modules together and runs
//! the event loop. Each concern lives in its own module:
//!
//! - [`entry`]      – the directory-entry data model
//! - [`filesystem`] – reading directories and file previews
//! - [`app`]        – application state and navigation logic
//! - [`input`]      – mapping key events to commands
//! - [`ui`]         – rendering state to the terminal
//! - [`terminal`]   – entering and restoring terminal modes

mod app;
mod entry;
mod filesystem;
mod input;
mod terminal;
mod ui;

use std::{error::Error, time::Duration};

use crossterm::event::{self, Event};

use app::App;
use terminal::TerminalGuard;

/// Poll interval for input; also caps how long the UI can lag behind a resize.
const TICK: Duration = Duration::from_millis(250);

fn main() {
    if let Err(error) = run() {
        eprintln!("filebrowser: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut terminal = terminal::setup()?;
    let _guard = TerminalGuard;
    let mut app = App::new();

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if app.should_quit() {
            break;
        }

        if event::poll(TICK)? {
            if let Event::Key(key_event) = event::read()? {
                input::handle_key(&mut app, key_event);
            }
        }
    }

    Ok(())
}
