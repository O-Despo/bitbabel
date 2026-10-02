//! The start screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{Row, SIZES, SetupForm};

const BOX_WIDTH: u16 = 64;
const BOX_HEIGHT: u16 = 10;

/// Draws the size row and the start button in a box in the middle of `area`.
pub fn draw(frame: &mut Frame, area: Rect, form: &SetupForm) {
    let [row] = Layout::vertical([Constraint::Length(BOX_HEIGHT)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(BOX_WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let focused = Style::default().add_modifier(Modifier::REVERSED);
    let mut size_row = vec![Span::raw("Library  ")];
    for size in SIZES {
        let name = size.name().unwrap_or("custom");
        let text = format!(" {name} ({} B) ", size.page_len());
        if size == form.size() {
            let style = if form.focus() == Row::Size {
                focused
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            };
            size_row.push(Span::styled(text, style));
        } else {
            size_row.push(Span::raw(text));
        }
    }
    let key = if form.has_key() {
        "custom key (from the command line)"
    } else {
        "canonical (the public key)"
    };
    let start_style = if form.focus() == Row::Start {
        focused
    } else {
        Style::default()
    };
    let lines = vec![
        Line::raw(""),
        Line::from(size_row),
        Line::raw(""),
        Line::raw(format!("Key      {key}")),
        Line::raw(""),
        Line::from(Span::styled(" [ Explore → page 0 ] ", start_style)).centered(),
        Line::raw(""),
        Line::raw("←/→ size   ↑/↓ move   enter start   esc quit").centered(),
    ];
    let block = Block::bordered().title(" bitbabel · choose a library ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
