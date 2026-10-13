//! Shared helpers for the app tests: fake random sources, ready-made apps and key presses.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Mode};
use crate::config::TuiConfig;
use crate::files::{Files, MemFiles};
use crate::random::RandomError;
use bitbabel_core::{LibraryConfig, PageIndex};

/// Fills every byte with `0xAB`, so a "random" index is easy to predict.
pub fn fake_random(buf: &mut [u8]) -> Result<(), RandomError> {
    buf.fill(0xAB);
    Ok(())
}

/// A source that always fails.
pub fn failing_random(_: &mut [u8]) -> Result<(), RandomError> {
    Err(RandomError::new("blocked"))
}

/// Fills each call with a different byte, to tell two seeds apart. The counter is per
/// thread, and every test runs on its own thread, so it always starts at 1 and tests cannot
/// disturb each other.
pub fn counting_random(buf: &mut [u8]) -> Result<(), RandomError> {
    use std::cell::Cell;
    thread_local! {
        static NEXT: Cell<u8> = const { Cell::new(1) };
    }
    NEXT.with(|next| {
        buf.fill(next.get());
        next.set(next.get().wrapping_add(1));
    });
    Ok(())
}

/// An app exploring the small library, with no files.
pub fn small_app() -> App {
    exploring(LibraryConfig::SMALL)
}

/// An app exploring the medium library, with no files.
pub fn medium_app() -> App {
    exploring(LibraryConfig::MEDIUM)
}

fn exploring(size: LibraryConfig) -> App {
    let config = TuiConfig {
        size: Some(size),
        key: None,
    };
    App::new(config, fake_random, Box::new(MemFiles::default()))
}

/// A key press with no modifiers.
pub fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Types `text` one key at a time.
pub fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(press(KeyCode::Char(c)));
    }
}

/// The index of the page being explored.
pub fn index_of(app: &App) -> PageIndex {
    let Mode::Explore(session) = app.mode() else {
        panic!("should be exploring");
    };
    session.index().clone()
}

/// The first visible line of the page being explored.
pub fn scroll_of(app: &App) -> usize {
    let Mode::Explore(session) = app.mode() else {
        panic!("should be exploring");
    };
    session.scroll()
}

/// The status line message, if there is one.
pub fn status_of(app: &App) -> Option<crate::app::session::Status> {
    let Mode::Explore(session) = app.mode() else {
        panic!("should be exploring");
    };
    session.status().cloned()
}

/// A fake file system the test can keep a handle on after giving it to the app.
pub struct SharedFiles(pub std::sync::Arc<MemFiles>);

impl Files for SharedFiles {
    fn read(&self, path: &std::path::Path, max: usize) -> std::io::Result<Vec<u8>> {
        self.0.read(path, max)
    }

    fn write_new(&self, path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
        self.0.write_new(path, bytes)
    }
}
