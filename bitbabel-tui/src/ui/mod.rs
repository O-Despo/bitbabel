//! Drawing only. It reads the [`App`] and never changes it.

mod explore;
mod setup;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};

/// The smallest terminal every screen fits in.
pub const MIN_WIDTH: u16 = 80;
/// See [`MIN_WIDTH`].
pub const MIN_HEIGHT: u16 = 24;

/// Draws the screen for the app's current mode, or a message if the terminal is too small.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area);
        return;
    }
    match app.mode() {
        Mode::Setup(form) => setup::draw(frame, area, form),
        Mode::Explore(session) => explore::draw(frame, area, session),
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let message = format!(
        "terminal too small (need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{})",
        area.width, area.height
    );
    frame.render_widget(Paragraph::new(message), area);
}

#[cfg(test)]
mod test {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::config::TuiConfig;
    use crate::files::MemFiles;
    use crate::random::RandomError;
    use bitbabel_core::LibraryConfig;

    fn fake_random(buf: &mut [u8]) -> Result<(), RandomError> {
        buf.fill(0);
        Ok(())
    }

    /// The screen as text, one line per row.
    fn render(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn too_small_says_so_in_every_mode() {
        let setup = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        );
        let explore = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        for app in [setup, explore] {
            assert!(render(&app, 79, 24).contains("terminal too small (need 80x24, have 79x24)"));
            assert!(render(&app, 80, 23).contains("terminal too small (need 80x24, have 80x23)"));
            assert!(!render(&app, 80, 24).contains("too small"));
        }
    }

    #[test]
    fn explore_shows_the_indicator_in_the_title() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        let title = screen.lines().next().unwrap();
        assert!(title.contains("medium · canonical"));
        assert!(title.contains("no bookmark file"));
    }

    #[test]
    fn setup_lists_the_sizes_and_the_start_button() {
        let app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        for text in [
            "small (16 B)",
            "medium (3200 B)",
            "large (6400 B)",
            "canonical",
            "Explore → page 0",
        ] {
            assert!(screen.contains(text), "missing {text:?}");
        }
    }

    #[test]
    fn a_resize_is_drawn_at_the_new_size() {
        let mut app = App::new(
            TuiConfig::default(),
            fake_random,
            Box::new(MemFiles::default()),
        );
        assert!(render(&app, 60, 20).contains("too small"));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(render(&app, 120, 40).contains("medium · canonical"));
    }

    #[test]
    fn setup_shows_the_key_file_check_inline() {
        let files = MemFiles::default().with("my.key", &[1; 32]);
        let mut app = App::new(TuiConfig::default(), fake_random, Box::new(files));
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Down);
        assert!(render(&app, 80, 24).contains("path:"));

        for c in "my.key".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert!(render(&app, 80, 24).contains("✓ loaded a576-7c85-fdef-3c24"));

        press(&mut app, KeyCode::Up);
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Enter);
        assert!(render(&app, 80, 24).contains("✗ cannot read my.keyx"));
    }

    #[test]
    fn explore_indicator_shows_the_fingerprint_not_the_key() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: Some(bitbabel_core::Key::from_bytes([1; 32])),
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        assert!(
            screen
                .lines()
                .next()
                .unwrap()
                .contains("small · key a576-7c85-fdef-3c24")
        );
    }

    #[test]
    fn explore_draws_the_index_address_and_page() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        let rows: Vec<&str> = screen.lines().collect();
        assert!(rows[0].contains("small · canonical · text"));
        assert!(rows[1].contains("index    "));
        assert!(rows[1].contains("(16 B)"));
        assert!(rows[2].contains("address  hexagon 0 · wall 1 · shelf 1 · volume 1"));
        // The one page line starts with its offset in the gutter.
        assert!(rows[3].contains("0000 "));
        assert!(rows[21].contains("1 lines"));
        assert!(rows[22].contains("q quit"));
    }

    #[test]
    fn explore_draws_as_many_page_lines_as_fit() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        let rows: Vec<&str> = screen.lines().collect();
        // 24 rows less the border, index, address, status and hint rows.
        assert!(rows[3].contains("0000 "));
        assert!(rows[20].contains("0440 "));
        assert!(!rows[21].contains("0480 "));
    }

    #[test]
    fn setup_draws_a_start_error_in_place_of_the_hint() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::new(15, 8)),
                key: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        );
        let screen = render(&app, 80, 24);
        assert!(screen.contains("✗ page length must be even and non-zero, got 15"));
        assert!(!screen.contains("enter start"));
    }
}
