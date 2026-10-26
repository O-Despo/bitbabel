//! The one-line prompt, drawn over the page.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::Prompt;

const WIDTH: u16 = 60;
const HEIGHT: u16 = 9;

/// Draws `prompt` in a box in the middle of `area`.
pub fn draw(frame: &mut Frame, area: Rect, prompt: &Prompt) {
    let [row] = Layout::vertical([Constraint::Length(HEIGHT)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let error = match prompt.error() {
        Some(error) => Line::styled(format!("✗ {error}"), Style::new().fg(Color::Red)),
        None => Line::raw(""),
    };
    let lines = vec![
        Line::raw(prompt.label()),
        Line::raw(format!("> {}▏", prompt.input())),
        error,
        Line::styled(prompt.footer(), Style::new().add_modifier(Modifier::DIM)),
    ];
    let block = Block::bordered().title(format!(" {} ", prompt.title()));
    frame.render_widget(Clear, area);
    // Long labels and messages wrap onto the lines the box has room for.
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
