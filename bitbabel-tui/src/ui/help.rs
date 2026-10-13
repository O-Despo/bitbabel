//! The help screen: every key and what it does, from the key map, in two columns.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::keymap;

/// The box is this wide, so it fits the smallest terminal (80 columns) with a margin.
const WIDTH: u16 = 78;
const _: () = assert!(WIDTH <= crate::ui::MIN_WIDTH);
/// The width of one column: half of the inside of the box.
const COLUMN_WIDTH: usize = (WIDTH as usize - 2) / 2;
/// The width of the key part of an entry, so the descriptions line up.
const KEY_WIDTH: usize = 7;

/// One entry as a line: the key in bold, then what it does.
fn entry_line(key: String, what: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{key:<KEY_WIDTH$}"),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::raw(what),
    ])
}

/// Draws the key list in a box in the middle of `area`. The entries run down the left column,
/// then down the right.
pub fn draw(frame: &mut Frame, area: Rect) {
    let entries = keymap::help_entries();
    let per_column = entries.len().div_ceil(2);
    // The rows of one column, the hint line, and the two border rows.
    let height = (per_column as u16 + 3).min(area.height);
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let block = Block::bordered().title(" keys ");
    let inside = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let [columns, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inside);
    let [left, right] =
        Layout::horizontal([Constraint::Length(COLUMN_WIDTH as u16); 2]).areas(columns);
    let mut lines: Vec<Line> = entries
        .into_iter()
        .map(|(key, what)| entry_line(key, what))
        .collect();
    let right_lines = lines.split_off(per_column);
    frame.render_widget(Paragraph::new(lines), left);
    frame.render_widget(Paragraph::new(right_lines), right);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "any key closes",
            Style::new().add_modifier(Modifier::DIM),
        )),
        footer,
    );
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_entry_fits_its_column() {
        for (key, what) in keymap::help_entries() {
            let width = KEY_WIDTH.max(key.chars().count() + 1) + what.chars().count();
            assert!(
                width <= COLUMN_WIDTH,
                "{key}: {what} is {width}, the column is {COLUMN_WIDTH}"
            );
        }
    }

    #[test]
    fn the_box_fits_the_smallest_terminal_with_room_for_more_keys() {
        // Ten more keys would still fit in 24 rows: 2 borders, a hint line, and the rows.
        let entries = keymap::help_entries().len() + 10;
        assert!(entries.div_ceil(2) + 3 <= 24);
    }
}
