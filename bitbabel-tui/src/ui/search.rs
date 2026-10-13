//! The search overlay, drawn over the page.

use bitbabel_core::LibraryConfig;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};

use super::explore::shorten;
use crate::app::SearchForm;
use crate::render::View;

const WIDTH: u16 = 68;
/// The rows that are not result rows: the needle line and the error line.
const HEADER_ROWS: usize = 2;
/// The key line at the bottom of the box.
const FOOTER_ROWS: usize = 1;
/// The top and bottom border.
const BORDER_ROWS: usize = 2;
/// The box is tall enough for the header, ten results, the footer and the border.
const HEIGHT: u16 = (BORDER_ROWS + HEADER_ROWS + crate::app::RESULTS + FOOTER_ROWS) as u16;

/// How much of an index is shown at each end.
const ENDS: usize = 12;

/// Draws `form` in a box in the middle of `area`. `view` is the encoding the needle is in.
pub fn draw(frame: &mut Frame, area: Rect, form: &SearchForm, view: View, size: LibraryConfig) {
    let [row] = Layout::vertical([Constraint::Length(HEIGHT)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let count = match form.byte_count(view) {
        Some(1) => "  (1 byte)".to_string(),
        Some(n) => format!("  ({n} bytes)"),
        None => String::new(),
    };
    let mut lines = vec![Line::raw(format!("needle: {}▏{count}", form.input()))];
    lines.push(match form.error() {
        Some(error) => Line::styled(format!("✗ {error}"), Style::new().fg(Color::Red)),
        None => Line::raw(""),
    });
    for (at, hit) in form.hits().iter().enumerate() {
        let base64 = hit.index().to_base64();
        let shown = if size == LibraryConfig::SMALL {
            base64
        } else {
            shorten(&base64, ENDS)
        };
        let text = format!("{:>2}  at 0x{:02x}   index {shown}", at + 1, hit.start());
        let style = if at == form.selected() {
            Style::new().add_modifier(Modifier::REVERSED)
        } else if form.is_fresh(view) {
            Style::new()
        } else {
            // The needle was edited, so these are for the old one.
            Style::new().add_modifier(Modifier::DIM)
        };
        lines.push(Line::styled(text, style));
    }
    // Pad to the full height so the key line sits on the last row inside the border.
    while lines.len() < usize::from(HEIGHT) - BORDER_ROWS - FOOTER_ROWS {
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled(
        "enter search/open   ↑/↓ pick   ctrl+r new set   esc close",
        Style::new().add_modifier(Modifier::DIM),
    ));

    let block = Block::bordered().title(format!(" search · {} ", view.name()));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
