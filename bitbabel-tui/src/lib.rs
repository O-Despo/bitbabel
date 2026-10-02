//! The terminal explorer for the BitBabel library. It owns all terminal IO.
//!
//! [`run`] takes a [`TuiConfig`] and returns when the user quits.

mod app;
mod config;
mod error;
mod random;
mod ui;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

use app::{App, Effect};
pub use config::TuiConfig;
pub use error::TuiError;
pub use random::{RandomError, RandomFn, os_random};

/// Runs the explorer until the user quits.
///
/// The terminal is put in raw mode on the alternate screen and put back on every way out,
/// including a panic.
///
/// # Errors
///
/// [`TuiError::Terminal`] if the terminal cannot be set up, drawn on or read from.
pub fn run(config: TuiConfig) -> Result<(), TuiError> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut App::new(config, os_random));
    let restored = ratatui::try_restore();
    result?;
    restored?;
    Ok(())
}

/// Draw, wait for an event, handle it, run its effects. Resizes fall through to a redraw.
fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<(), TuiError> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        for effect in app.handle_key(key) {
            match effect {
                Effect::Quit => return Ok(()),
                // Arrives with the copy keys.
                Effect::Clipboard(_) => {}
            }
        }
    }
}
