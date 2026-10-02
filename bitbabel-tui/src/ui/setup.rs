//! The start screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::fingerprint_text;
use crate::app::{KeySource, KeyStatus, Row, SIZES, SetupForm, loaded_text};

const BOX_WIDTH: u16 = 64;
const BOX_HEIGHT: u16 = 14;

/// Draws the size, key and start rows in a box in the middle of `area`.
pub fn draw(frame: &mut Frame, area: Rect, form: &SetupForm) {
    let [row] = Layout::vertical([Constraint::Length(BOX_HEIGHT)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::horizontal([Constraint::Length(BOX_WIDTH)])
        .flex(Flex::Center)
        .areas(row);

    let mut lines = vec![Line::raw(""), size_row(form), Line::raw(""), key_row(form)];
    if form.rows().contains(&Row::Path) {
        lines.push(path_row(form));
        lines.push(status_row(form));
    }
    lines.push(Line::raw(""));
    lines.push(start_row(form));
    lines.push(Line::raw(""));
    lines.push(Line::raw("←/→ choose   ↑/↓ move   enter start   esc quit").centered());

    let block = Block::bordered().title(" bitbabel · choose a library ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// The style of a choice: reversed when its row has focus, bold when only chosen.
fn chosen(form: &SetupForm, row: Row) -> Style {
    if form.focus() == row {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

fn size_row(form: &SetupForm) -> Line<'static> {
    let mut spans = vec![Span::raw("Library  ")];
    for size in SIZES {
        let name = size.name().unwrap_or("custom");
        let text = format!(" {name} ({} B) ", size.page_len());
        if size == form.size() {
            spans.push(Span::styled(text, chosen(form, Row::Size)));
        } else {
            spans.push(Span::raw(text));
        }
    }
    Line::from(spans)
}

fn key_row(form: &SetupForm) -> Line<'static> {
    if let Some(key) = form.fixed_key() {
        return Line::raw(format!(
            "Key      from the command line, {}",
            fingerprint_text(key)
        ));
    }
    let mut spans = vec![Span::raw("Key      ")];
    for (source, name) in [
        (KeySource::Canonical, " canonical "),
        (KeySource::File, " key file "),
    ] {
        if source == form.source() {
            spans.push(Span::styled(name, chosen(form, Row::Key)));
        } else {
            spans.push(Span::raw(name));
        }
    }
    Line::from(spans)
}

fn path_row(form: &SetupForm) -> Line<'static> {
    let cursor = if form.focus() == Row::Path { "▏" } else { "" };
    Line::raw(format!("         path: {}{cursor}", form.path()))
}

fn status_row(form: &SetupForm) -> Line<'static> {
    let text = match form.status() {
        KeyStatus::Unchecked => String::new(),
        KeyStatus::Loaded(key) => format!("✓ {}", loaded_text(key)),
        KeyStatus::Failed(message) => format!("✗ {message}"),
    };
    Line::raw(format!("               {text}"))
}

fn start_row(form: &SetupForm) -> Line<'static> {
    let style = if form.focus() == Row::Start {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(Span::styled(" [ Explore → page 0 ] ", style)).centered()
}
