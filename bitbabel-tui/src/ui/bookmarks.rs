//! The `B` bookmark list, drawn over the page.

use bitbabel_core::LibraryConfig;
use bitbabel_file::Bookmark;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::explore::shorten;
use crate::app::Session;
use crate::bookmarks::{Bookmarks, short_name};
use crate::clock::format_utc;

/// The same width as the search box (§13.3b).
const WIDTH: u16 = 76;
/// The top and bottom border.
const BORDER_ROWS: usize = 2;
/// The key line at the bottom of the box.
const FOOTER_ROWS: usize = 1;
/// How much of an index is shown at each end, as on the search rows.
const ENDS: usize = 12;
/// The cells the UTC date column holds (`2026-10-01 14:20Z`).
const DATE_CELLS: usize = 17;
/// The cells before the label: ` N  date  `.
const BEFORE_LABEL: usize = 1 + 2 + 2 + DATE_CELLS + 2;
/// The two spaces between the label's column and the index.
const LABEL_GAP: usize = 2;

/// Draws the list in a box in the middle of `area`. `session.viewport()` rows are visible;
/// the same number the keys scroll through.
pub fn draw(frame: &mut Frame, area: Rect, session: &Session) {
    let rows = session.viewport();
    let height = (rows + BORDER_ROWS + FOOTER_ROWS) as u16;
    let [row] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let list = session.bookmark_list();
    let mut lines: Vec<Line> = Vec::new();
    match session.bookmarks() {
        Bookmarks::Locked(locked) if locked.count() > 0 => {
            let size = session.library().size();
            let inside = usize::from(WIDTH) - 2;
            for (at, mark) in locked
                .file()
                .bookmarks()
                .iter()
                .enumerate()
                .skip(list.top())
                .take(rows)
            {
                let line = row_line(at, mark, size, inside);
                lines.push(if at == list.selected() {
                    line.style(Style::new().add_modifier(Modifier::REVERSED))
                } else {
                    line
                });
            }
        }
        // The list only opens over a file, so a free session cannot get here; an empty file
        // can, and shows one dim line.
        _ => lines.push(Line::styled(
            "no bookmarks",
            Style::new().add_modifier(Modifier::DIM),
        )),
    }
    // Pad so the key line sits on the last row inside the border.
    while lines.len() < rows {
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled(
        "enter open   r rename   esc close",
        Style::new().add_modifier(Modifier::DIM),
    ));

    let name = match session.bookmarks() {
        Bookmarks::Locked(locked) => short_name(&locked.name()),
        Bookmarks::Free => "no file".to_string(),
    };
    let block = Block::bordered().title(format!(
        " bookmarks · {name} · {} ",
        session.library().label()
    ));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// One row: ` N  date  label  index`. The label takes what is left, so the index stays whole.
fn row_line(at: usize, mark: &Bookmark, size: LibraryConfig, inside: usize) -> Line<'static> {
    let base64 = mark.index().to_base64();
    let index = if size == LibraryConfig::SMALL {
        base64
    } else {
        shorten(&base64, ENDS)
    };
    let label_room = inside.saturating_sub(BEFORE_LABEL + LABEL_GAP + Span::raw(&index).width());
    let label = head_that_fits(mark.label(), label_room);
    // Pad the label's column so the index lines up on every row.
    let pad = " ".repeat(label_room - Span::raw(&label).width());
    Line::raw(format!(
        " {:>2}  {}  {label}{pad}  {index}",
        at + 1,
        format_utc(mark.time())
    ))
}

/// `text` if it is at most `max` cells wide, else its start with `…` at the end, as wide as
/// will fit. Width is in terminal cells (§13.3b), like the needle tail in `ui/search.rs`.
fn head_that_fits(text: &str, max: usize) -> String {
    if Span::raw(text).width() <= max {
        return text.to_string();
    }
    // The `…` takes one cell.
    let room = max.saturating_sub(1);
    let mut kept = String::new();
    let mut width = 0;
    for c in text.chars() {
        let cell = Span::raw(c.to_string()).width();
        if width + cell > room {
            break;
        }
        width += cell;
        kept.push(c);
    }
    format!("{kept}…")
}

#[cfg(test)]
mod test {
    use super::*;

    fn width(text: &str) -> usize {
        Span::raw(text).width()
    }

    #[test]
    fn a_label_that_fits_is_shown_whole() {
        assert_eq!(head_that_fits("a word", 10), "a word");
        assert_eq!(head_that_fits("", 10), "");
    }

    #[test]
    fn a_longer_label_keeps_its_start_before_an_ellipsis() {
        let shown = head_that_fits("the quick brown fox", 10);
        assert_eq!(shown, "the quick…");
        assert_eq!(width(&shown), 10);
    }

    #[test]
    fn wide_characters_count_two_cells() {
        let shown = head_that_fits("日本語日本語", 5);
        assert_eq!(shown, "日本…");
        assert!(width(&shown) <= 5);
    }

    #[test]
    fn no_room_still_gives_the_ellipsis_as_the_needle_tail_does() {
        assert_eq!(head_that_fits("abc", 1), "…");
        assert_eq!(head_that_fits("abc", 0), "…");
    }
}
