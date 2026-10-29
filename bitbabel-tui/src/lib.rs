//! The terminal explorer for the BitBabel library. It owns all terminal IO.
//!
//! [`run`] takes a [`TuiConfig`] and returns when the user quits.

mod app;
mod bookmarks;
mod clock;
mod config;
mod error;
mod files;
mod history;
mod keymap;
mod random;
mod render;
mod ui;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event};

use app::{App, Effect};
pub use bookmarks::{LockedBookmarks, SaveError, StartError};
pub use config::TuiConfig;
pub use error::TuiError;
pub use files::{Files, StdFiles};
pub use random::{RandomError, RandomFn, os_random};

/// Runs the explorer until the user quits.
///
/// The terminal is put in raw mode on the alternate screen and put back on every way out,
/// including a panic.
///
/// # Errors
///
/// - [`TuiError::Bookmarks`] if the bookmark file in `config` cannot be used. This is found
///   before the terminal is touched, so the message prints in the normal terminal.
/// - [`TuiError::Terminal`] if the terminal cannot be set up, drawn on or read from.
pub fn run(config: TuiConfig) -> Result<(), TuiError> {
    let mut app = App::new(config, os_random, Box::new(StdFiles))?;
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, &mut app);
    let restored = ratatui::try_restore();
    result?;
    restored?;
    Ok(())
}

/// Draw, wait for an event, handle it, run its effects. A resize tells the app the new
/// height and the next draw uses it.
fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<(), TuiError> {
    app.set_terminal_height(terminal.size()?.height);
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        match event::read()? {
            Event::Key(key) => {
                if app.handle_key(key).contains(&Effect::Quit) {
                    return Ok(());
                }
            }
            Event::Resize(_, height) => app.set_terminal_height(height),
            _ => {}
        }
    }
}
