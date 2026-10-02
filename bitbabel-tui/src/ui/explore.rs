//! The explore screen. A stub until pages are drawn.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::app::App;

/// Draws the title bar with the library indicator, and a hint.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let library = app
        .library()
        .map(|library| library.label())
        .unwrap_or_default();
    let block = Block::bordered()
        .title(format!(" {library} "))
        .title(Line::from(" no bookmark file ").right_aligned());
    frame.render_widget(Paragraph::new("s start over   q quit").block(block), area);
}
