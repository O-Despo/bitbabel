//! The help screen: every key and what it does, from the key map.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::keymap;

const WIDTH: u16 = 60;
/// The width of the key column, so the descriptions line up.
const KEY_WIDTH: usize = 8;

/// Draws the key list in a box in the middle of `area`.
pub fn draw(frame: &mut Frame, area: Rect) {
    let entries = keymap::help_entries();
    // The entries, the hint line, and the two border rows.
    let height = (entries.len() as u16 + 3).min(area.height);
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let mut lines: Vec<Line> = entries
        .into_iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<KEY_WIDTH$}"),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(what),
            ])
        })
        .collect();
    lines.push(Line::styled(
        "any key closes",
        Style::new().add_modifier(Modifier::DIM),
    ));
    let block = Block::bordered().title(" keys ");
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
