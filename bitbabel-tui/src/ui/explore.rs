//! The explore screen: the library in the title, the index and its address, the page, and a
//! status and hint line.

use bitbabel_core::{LibraryConfig, PageIndex};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::Session;
use crate::render::DisplayLine;

/// How many characters of an index or hexagon are kept at each end when it is shortened.
const ENDS: usize = 12;

/// Width of the offset gutter: four hex digits and a space.
const GUTTER: u16 = 5;

/// Draws the explore screen into `area`.
pub fn draw(frame: &mut Frame, area: Rect, session: &Session) {
    let block = Block::bordered()
        .title(format!(
            " {} · {} ",
            session.library().label(),
            session.view().name()
        ))
        .title(Line::from(" no bookmark file ").right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [index_row, address_row, page_area, status_row, hint_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let size = session.library().size();
    frame.render_widget(Line::raw(index_line(session.index(), size)), index_row);
    frame.render_widget(Line::raw(address_line(session.index(), size)), address_row);
    draw_page(frame, page_area, session);
    frame.render_widget(
        Line::styled(
            format!("{} lines", session.lines().len()),
            Style::new().add_modifier(Modifier::DIM),
        ),
        status_row,
    );
    frame.render_widget(
        Line::styled(
            "s start over   q quit",
            Style::new().add_modifier(Modifier::DIM),
        ),
        hint_row,
    );
}

/// The page lines that fit, each with its offset in the gutter.
fn draw_page(frame: &mut Frame, area: Rect, session: &Session) {
    let [gutter_area, text_area] =
        Layout::horizontal([Constraint::Length(GUTTER), Constraint::Min(1)]).areas(area);
    let mut gutter = Vec::new();
    let mut lines = Vec::new();
    for line in session.lines().iter().take(usize::from(area.height)) {
        gutter.push(Line::styled(
            format!("{:04x} ", line.offset),
            Style::new().dim(),
        ));
        lines.push(page_line(line));
    }
    frame.render_widget(Paragraph::new(gutter), gutter_area);
    frame.render_widget(Paragraph::new(lines), text_area);
}

/// One page line, with highlighted runs reversed.
fn page_line(line: &DisplayLine) -> Line<'static> {
    let highlight = Style::new().fg(Color::Black).bg(Color::Yellow);
    Line::from(
        line.segments
            .iter()
            .map(|segment| {
                if segment.highlighted {
                    Span::styled(segment.text.clone(), highlight)
                } else {
                    Span::raw(segment.text.clone())
                }
            })
            .collect::<Vec<_>>(),
    )
}

/// `text` as is if it is short, else its first and last `keep` characters around `…`.
fn shorten(text: &str, keep: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= keep * 2 + 1 {
        return text.to_string();
    }
    let start: String = chars[..keep].iter().collect();
    let end: String = chars[chars.len() - keep..].iter().collect();
    format!("{start}…{end}")
}

/// `index    n0aMceL…xQ2p  (3200 B)`: the whole index on the small size, else its two ends.
fn index_line(index: &PageIndex, size: LibraryConfig) -> String {
    let base64 = index.to_base64();
    let shown = if size == LibraryConfig::SMALL {
        base64
    } else {
        shorten(&base64, ENDS)
    };
    format!("index    {shown}  ({} B)", index.as_bytes().len())
}

/// `address  hexagon 5f21…9c1d · wall 2 · shelf 4 · volume 17`.
fn address_line(index: &PageIndex, size: LibraryConfig) -> String {
    let location = index.location();
    let hexagon = location.hexagon_hex();
    let hexagon = if size == LibraryConfig::SMALL {
        hexagon
    } else {
        shorten(&hexagon, ENDS)
    };
    format!(
        "address  hexagon {hexagon} · wall {} · shelf {} · volume {}",
        location.wall(),
        location.shelf(),
        location.volume()
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn short_text_is_kept_whole() {
        assert_eq!(shorten("abc", 12), "abc");
        assert_eq!(shorten(&"a".repeat(25), 12), "a".repeat(25));
    }

    #[test]
    fn long_text_keeps_its_two_ends() {
        let text = format!("{}{}", "a".repeat(12), "b".repeat(30));
        assert_eq!(
            shorten(&text, 12),
            format!("{}…{}", "a".repeat(12), "b".repeat(12))
        );
    }

    #[test]
    fn small_indexes_are_shown_in_full() {
        let index = PageIndex::zero(16);
        assert_eq!(
            index_line(&index, LibraryConfig::SMALL),
            "index    AAAAAAAAAAAAAAAAAAAAAA==  (16 B)"
        );
    }

    #[test]
    fn big_indexes_are_shortened_with_their_length() {
        let index = PageIndex::zero(3200);
        assert_eq!(
            index_line(&index, LibraryConfig::MEDIUM),
            "index    AAAAAAAAAAAA…AAAAAAAAAAA=  (3200 B)"
        );
    }

    #[test]
    fn the_address_line_names_hexagon_wall_shelf_and_volume() {
        assert_eq!(
            address_line(&PageIndex::zero(16), LibraryConfig::SMALL),
            "address  hexagon 0 · wall 1 · shelf 1 · volume 1"
        );
        assert_eq!(
            address_line(&PageIndex::from_u64(641, 16), LibraryConfig::SMALL),
            "address  hexagon 1 · wall 1 · shelf 1 · volume 2"
        );
    }

    #[test]
    fn a_big_hexagon_is_shortened_but_a_small_one_is_not() {
        let last = PageIndex::from_bytes(vec![0xff; 3200]);
        let line = address_line(&last, LibraryConfig::MEDIUM);
        assert!(line.contains('…'));
        assert!(line.ends_with("wall 2 · shelf 3 · volume 32"));

        let small_last = PageIndex::from_bytes(vec![0xff; 16]);
        assert!(!address_line(&small_last, LibraryConfig::SMALL).contains('…'));
    }
}
