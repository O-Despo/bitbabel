//! The search overlay, drawn over the page.

use bitbabel_core::LibraryConfig;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::explore::shorten;
use crate::app::SearchForm;
use crate::render::View;

/// Wide enough for a good part of a needle, and it still fits the smallest terminal (80
/// columns) with a margin.
const WIDTH: u16 = 76;
/// The start of the needle line.
const NEEDLE_LABEL: &str = "needle: ";
/// The mark where typing continues, at the end of the needle.
const CURSOR: &str = "▏";
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

/// `text` if it is at most `max` cells wide, else its end with `…` in front, as wide as will fit.
/// Width is in terminal cells, so wide characters such as CJK count twice.
fn tail_that_fits(text: &str, max: usize) -> String {
    if Span::raw(text).width() <= max {
        return text.to_string();
    }
    // The `…` takes one cell.
    let room = max.saturating_sub(1);
    let mut kept: Vec<char> = Vec::new();
    let mut width = 0;
    for c in text.chars().rev() {
        let cell = Span::raw(c.to_string()).width();
        if width + cell > room {
            break;
        }
        width += cell;
        kept.push(c);
    }
    kept.reverse();
    format!("…{}", kept.into_iter().collect::<String>())
}

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
    // What is left of the line once the label, the cursor and the byte count have their room.
    // A longer needle shows its end, where typing is, so the count is never pushed off.
    let inside = usize::from(WIDTH) - 2;
    let used = [NEEDLE_LABEL, CURSOR, &count].map(|text| Span::raw(text).width());
    let room = inside.saturating_sub(used.iter().sum::<usize>());
    let needle = tail_that_fits(form.input(), room);
    let mut lines = vec![Line::raw(format!("{NEEDLE_LABEL}{needle}{CURSOR}{count}"))];
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

#[cfg(test)]
mod test {
    use super::*;

    fn width(text: &str) -> usize {
        Span::raw(text).width()
    }

    #[test]
    fn text_that_fits_is_unchanged() {
        assert_eq!(tail_that_fits("hello", 5), "hello");
        assert_eq!(tail_that_fits("", 0), "");
    }

    #[test]
    fn longer_text_keeps_its_end_behind_an_ellipsis_that_fits() {
        let shown = tail_that_fits("the quick brown fox", 8);
        assert_eq!(shown, "…own fox");
        assert_eq!(width(&shown), 8);
        assert_eq!(tail_that_fits("abcdef", 6), "abcdef");
        assert_eq!(tail_that_fits("abcdefg", 6), "…cdefg");
    }

    #[test]
    fn wide_characters_count_two_cells_so_the_result_never_overflows() {
        let shown = tail_that_fits("日本語日本語", 8);
        assert!(width(&shown) <= 8, "{shown}");
        assert!(shown.starts_with('…'));
        assert!(shown.ends_with("日本語"));
        // One cell short of a pair is left empty rather than overflowing.
        assert_eq!(tail_that_fits("日本語", 4), "…語");
    }

    #[test]
    fn no_room_at_all_still_gives_the_ellipsis() {
        assert_eq!(tail_that_fits("abc", 0), "…");
        assert_eq!(tail_that_fits("abc", 1), "…");
    }
}
