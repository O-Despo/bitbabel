//! Drawing only. It reads the [`App`] and never changes it.

use ratatui::Frame;
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Mode};

/// Draws the screen for the app's current mode.
pub fn draw(frame: &mut Frame, app: &App) {
    match app.mode() {
        Mode::Explore => draw_explore(frame),
    }
}

fn draw_explore(frame: &mut Frame) {
    let block = Block::bordered().title("bitbabel");
    let text = Paragraph::new("q to quit").block(block);
    frame.render_widget(text, frame.area());
}
