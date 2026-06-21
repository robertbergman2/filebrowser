//! Terminal lifecycle: entering and restoring raw/alternate-screen mode.
//!
//! [`TerminalGuard`] guarantees the terminal is restored on the way out, even
//! if rendering or the event loop returns an error or panics.

use std::{
    error::Error,
    io::{self, Stdout},
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Restores the terminal when dropped.
pub struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = teardown();
    }
}

/// Put the terminal into raw, alternate-screen mode and return a ratatui
/// terminal. On any failure the partial setup is undone before returning.
pub fn setup() -> Result<Tui, Box<dyn Error>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
        let _ = disable_raw_mode();
        return Err(Box::new(error));
    }

    let backend = CrosstermBackend::new(stdout);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = teardown();
            Err(Box::new(error))
        }
    }
}

/// Restore the terminal to its original mode, reporting the first error seen
/// while still attempting every restoration step.
pub fn teardown() -> io::Result<()> {
    let mut first_error = disable_raw_mode().err();
    let mut stdout = io::stdout();

    if let Err(error) = execute!(stdout, LeaveAlternateScreen, Show) {
        first_error.get_or_insert(error);
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
