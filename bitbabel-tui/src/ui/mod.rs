//! Drawing only. It reads the [`App`] and never changes it.

mod bookmarks;
mod explore;
mod help;
mod prompt;
mod search;
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
        Mode::Explore(session) => {
            explore::draw(frame, area, session);
            if let Some(open) = session.prompt() {
                prompt::draw(frame, area, open);
            }
            if session.search_is_open()
                && let Some(form) = session.search_form()
            {
                let size = session.library().size();
                search::draw(frame, area, form, session.view(), size);
            }
            if session.bookmark_list_is_open() {
                bookmarks::draw(frame, area, session);
            }
            if session.help_is_open() {
                help::draw(frame, area);
            }
        }
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
    use crate::app::test_util::fake_random;
    use crate::config::TuiConfig;
    use crate::files::MemFiles;
    use bitbabel_core::LibraryConfig;

    /// The screen's cells, with their styles.
    fn cells(app: &App, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// The screen as text, one line per row.
    fn render(app: &App, width: u16, height: u16) -> String {
        let buffer = cells(app, width, height);
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
        )
        .unwrap();
        let explore = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
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
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
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
        )
        .unwrap();
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
        )
        .unwrap();
        assert!(render(&app, 60, 20).contains("too small"));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(render(&app, 120, 40).contains("medium · canonical"));
    }

    #[test]
    fn setup_shows_the_key_file_check_inline() {
        let files = MemFiles::default().with("my.key", &[1; 32]);
        let mut app = App::new(TuiConfig::default(), fake_random, Box::new(files)).unwrap();
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
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
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
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let screen = render(&app, 80, 24);
        let rows: Vec<&str> = screen.lines().collect();
        assert!(rows[0].contains("small · canonical · text"));
        assert!(rows[1].contains("index    "));
        assert!(rows[1].contains("(16 B)"));
        assert!(rows[2].contains("address  hexagon 0 · wall 1 · shelf 1 · volume 1"));
        // The one page line starts with its offset in the gutter.
        assert!(rows[3].contains("0000 "));
        assert!(rows[21].contains("ok · line 1/1"));
        assert!(rows[22].contains("j/k scroll"));
        assert!(rows[22].contains("q quit"));
    }

    #[test]
    fn explore_draws_as_many_page_lines_as_fit() {
        let app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
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
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let screen = render(&app, 80, 24);
        assert!(screen.contains("✗ page length must be even and non-zero, got 15"));
        assert!(!screen.contains("enter start"));
    }

    #[test]
    fn scrolling_moves_the_page_lines_and_the_status() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        for _ in 0..5 {
            press(&mut app, KeyCode::Char('j'));
        }
        let rows: Vec<String> = render(&app, 80, 24).lines().map(String::from).collect();
        // Line 5 (offset 0x140) is now the first page row.
        assert!(rows[3].starts_with("│0140 "));
        assert!(rows[21].contains("ok · line 6/50"));

        press(&mut app, KeyCode::End);
        let rows: Vec<String> = render(&app, 80, 24).lines().map(String::from).collect();
        assert!(rows[21].contains("ok · line 33/50"));
        // The last page line (offset 49 * 0x40 = 0xc40) is on the last page row.
        assert!(rows[20].starts_with("│0c40 "));
    }

    #[test]
    fn the_page_rows_on_screen_match_what_the_app_calls_the_viewport() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::MEDIUM),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        app.set_terminal_height(30);
        let screen = render(&app, 80, 30);
        // Count rows that start with a gutter offset.
        let page_rows = screen
            .lines()
            .filter(|row| row.starts_with("│0") && row.chars().nth(5) == Some(' '))
            .count();
        let Mode::Explore(session) = app.mode() else {
            panic!("should be exploring");
        };
        assert_eq!(page_rows, session.viewport());
        assert_eq!(session.viewport(), 24);
    }

    /// An app locked to a bookmark file called `name`, for a private key, with `count`
    /// bookmarks.
    fn locked_app(name: &str, count: u64) -> App {
        use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
        let key = bitbabel_core::Key::from_bytes([1; 32]);
        let settings =
            BookmarkSettings::new(LibraryConfig::LARGE, BookmarkKey::Custom(key.fingerprint()))
                .unwrap();
        let mut file = BookmarkFile::new(settings);
        for n in 1..=count {
            let index = bitbabel_core::PageIndex::from_u64(n, LibraryConfig::LARGE.page_len());
            file.push(n, "", index).unwrap();
        }
        let files = MemFiles::default().with(name, &file.to_bytes());
        let config = TuiConfig {
            size: None,
            key: Some(key),
            bookmarks: Some(name.into()),
        };
        App::new(config, fake_random, Box::new(files)).unwrap()
    }

    #[test]
    fn the_title_says_there_is_no_bookmark_file_or_names_the_locked_one() {
        let free = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        assert!(
            render(&free, 80, 24)
                .lines()
                .next()
                .unwrap()
                .contains("no bookmark file")
        );

        let locked = render(&locked_app("favs.babelbookmark", 3), 80, 24);
        let title = locked.lines().next().unwrap();
        assert!(title.contains("large · key a576-7c85-fdef-3c24"), "{title}");
        assert!(title.contains("locked: favs (3)"), "{title}");
    }

    #[test]
    fn a_long_file_name_is_cut_so_the_whole_title_fits_in_80_columns() {
        // The widest title: a private key, the longest view name, a long file name, 4 digits.
        let mut app = locked_app("a-very-long-bookmark-file-name.babelbookmark", 1);
        for _ in 0..2 {
            app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        }
        let screen = render(&app, 80, 24);
        let title = screen.lines().next().unwrap();
        assert!(title.contains("locked: a-very-long-boo… (1)"), "{title}");
        assert!(title.contains("large · key a576-7c85-fdef-3c24"), "{title}");
        assert!(title.contains("base64"), "{title}");
    }

    #[test]
    fn the_first_bookmark_asks_for_a_file_then_a_label_that_says_it_locks_the_session() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        app.handle_key(key('b'));
        for c in "my-long-folder/favourites".chars() {
            app.handle_key(key(c));
        }
        let file_step = render(&app, 80, 24);
        assert!(file_step.contains("bookmark file"));
        assert!(file_step.contains("keep bookmarks in which file?"));
        assert!(file_step.contains("> my-long-folder/favourites"));
        assert!(file_step.contains("enter next   esc cancel"));

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // The long label question wraps inside the box instead of being cut off.
        let label_step = render(&app, 80, 24);
        assert!(label_step.contains("my-long-folder/favourites.babelbookmark"));
        assert!(label_step.contains("session to this library"));
        assert!(label_step.contains("enter save   esc back"));
    }

    #[test]
    fn the_help_screen_lists_every_key_from_the_key_map() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let page = render(&app, 80, 24);
        assert!(!page.contains("any key closes"));
        // The hint bar says where the help is.
        assert!(page.contains("q quit  ? help"));

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let screen = render(&app, 80, 24);
        assert!(screen.contains("any key closes"));
        for (key, what) in crate::keymap::help_entries() {
            assert!(screen.contains(what), "{key}: {what}");
        }
        assert!(screen.contains("Ctrl+d"));
        assert!(screen.contains("scroll down half a screen"));

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!render(&app, 80, 24).contains("any key closes"));
    }

    #[test]
    fn an_open_prompt_is_drawn_over_the_page_with_its_error() {
        let files = MemFiles::default().with("taken.txt", b"x");
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(files),
        )
        .unwrap();
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Char('x'));
        for c in "taken.txt".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let before = render(&app, 80, 24);
        assert!(before.contains("export index"));
        assert!(before.contains("> taken.txt"));
        assert!(!before.contains("already exists"));

        press(&mut app, KeyCode::Enter);
        assert!(render(&app, 80, 24).contains("✗ taken.txt already exists, choose another name"));
    }

    /// A locked app of `size` with `count` labelled marks, one minute apart.
    fn marks_app(size: LibraryConfig, count: usize) -> App {
        use crate::app::test_util::{MARKS, bookmarked_app};
        use bitbabel_core::PageIndex;
        use bitbabel_file::{BookmarkFile, BookmarkKey, BookmarkSettings};
        let settings = BookmarkSettings::new(size, BookmarkKey::Canonical).unwrap();
        let mut file = BookmarkFile::new(settings);
        for n in 0..count {
            let index = PageIndex::from_u64(n as u64 + 1, size.page_len());
            file.push(1_790_812_800 + (n as u64) * 60, &format!("mark {n}"), index)
                .unwrap();
        }
        let files = std::sync::Arc::new(MemFiles::default().with(MARKS, &file.to_bytes()));
        bookmarked_app(files)
    }

    #[test]
    fn the_bookmark_list_is_drawn_over_the_page_and_holds_the_selection() {
        use ratatui::style::Modifier;

        let mut app = marks_app(LibraryConfig::SMALL, 3);
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        app.handle_key(key('B'));
        let screen = render(&app, 80, 24);
        assert!(
            screen.contains("bookmarks · marks · small · canonical"),
            "{screen}"
        );
        assert!(screen.contains("1  2026-10-01 00:00Z  mark 0"), "{screen}");
        assert!(screen.contains("3  2026-10-01 00:02Z  mark 2"), "{screen}");
        assert!(screen.contains("enter open   esc close"), "{screen}");
        // The whole base64 of the small index is shown, not a shortened one.
        let index0 = bitbabel_core::PageIndex::from_u64(1, 16).to_base64();
        assert!(screen.contains(index0.as_str()), "{screen}");

        // The first row is reversed; j moves the reversal down.
        let selected_reversed = |app: &App, marker: &str| {
            let row = row_with(app, marker);
            cells(app, 80, 24)[(4, row)]
                .modifier
                .contains(Modifier::REVERSED)
        };
        assert!(selected_reversed(&app, "mark 0"));
        app.handle_key(key('j'));
        assert!(selected_reversed(&app, "mark 1"));
    }

    #[test]
    fn the_medium_list_shortens_the_index_like_the_search_rows() {
        let mut app = marks_app(LibraryConfig::MEDIUM, 2);
        app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
        let screen = render(&app, 80, 24);
        let whole = bitbabel_core::PageIndex::from_u64(1, 3200).to_base64();
        assert!(!screen.contains(whole.as_str()), "the whole index leaked");
        let head: String = whole.chars().take(12).collect();
        let tail: String = whole.chars().skip(whole.len() - 12).collect();
        assert!(
            screen.contains(format!("{head}…{tail}").as_str()),
            "{screen}"
        );
    }

    #[test]
    fn an_empty_bookmark_list_says_so() {
        let mut app = marks_app(LibraryConfig::SMALL, 0);
        app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE));
        assert!(render(&app, 80, 24).contains("no bookmarks"));
    }

    #[test]
    fn the_search_overlay_shows_the_view_the_byte_count_and_the_results() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Char('/'));
        for c in "hello".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let before = render(&app, 80, 24);
        assert!(before.contains("search · text"));
        assert!(before.contains("needle: hello"));
        assert!(before.contains("(5 bytes)"));

        press(&mut app, KeyCode::Enter);
        let after = render(&app, 80, 24);
        assert!(after.contains(" 1  at 0x"));
        assert!(after.contains("10  at 0x"));
        assert!(after.contains("enter search"));
    }

    fn searching_app() -> App {
        App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap()
    }

    #[test]
    fn a_long_needle_shows_its_end_and_keeps_the_byte_count() {
        let mut app = searching_app();
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        app.handle_key(key('/'));
        for c in "the quick brown fox jumps over the lazy dog and keeps on running".chars() {
            app.handle_key(key(c));
        }
        let screen = render(&app, 80, 24);
        let line = screen.lines().find(|l| l.contains("needle:")).unwrap();
        assert!(line.contains("needle: …"), "{line}");
        assert!(line.contains("keeps on running▏"), "{line}");
        assert!(line.contains("(64 bytes)"), "{line}");
        assert!(!line.contains("the quick"), "{line}");
        // The whole line, count included, stays inside the box.
        assert!(line.trim_end().ends_with('│'), "{line}");
    }

    #[test]
    fn a_short_needle_is_drawn_whole() {
        let mut app = searching_app();
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        app.handle_key(key('/'));
        for c in "hello".chars() {
            app.handle_key(key(c));
        }
        let screen = render(&app, 80, 24);
        let line = screen.lines().find(|l| l.contains("needle:")).unwrap();
        assert!(line.contains("needle: hello▏  (5 bytes)"), "{line}");
        assert!(!line.contains('…'), "{line}");
    }

    #[test]
    fn a_needle_of_wide_characters_keeps_the_byte_count_too() {
        let mut app = searching_app();
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        // The utf8 view takes the text as it is. `e` goes text, hex, base64, utf8.
        for _ in 0..3 {
            app.handle_key(key('e'));
        }
        app.handle_key(key('/'));
        for _ in 0..40 {
            app.handle_key(key('日'));
        }
        let screen = render(&app, 80, 24);
        let line = screen.lines().find(|l| l.contains("needle:")).unwrap();
        assert!(line.contains("(120 bytes)"), "{line}");
        assert!(line.contains("needle: …"), "{line}");
        assert!(line.trim_end().ends_with('│'), "{line}");
    }

    #[test]
    fn the_search_overlay_shows_a_needle_error_inline() {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Char('e')); // hex view
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('g'));
        press(&mut app, KeyCode::Enter);
        let screen = render(&app, 80, 24);
        assert!(screen.contains("search · hex"));
        assert!(screen.contains("✗ invalid hex digit 'g'"));
    }

    /// A small-library app with the search overlay open and `needle` typed.
    fn searching(needle: &str) -> App {
        let mut app = App::new(
            TuiConfig {
                size: Some(LibraryConfig::SMALL),
                key: None,
                bookmarks: None,
            },
            fake_random,
            Box::new(MemFiles::default()),
        )
        .unwrap();
        let press = |app: &mut App, code| {
            app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        };
        press(&mut app, KeyCode::Char('/'));
        for c in needle.chars() {
            press(&mut app, KeyCode::Char(c));
        }
        app
    }

    /// The text of the first screen row that contains `marker`, and its row number.
    fn row_with(app: &App, marker: &str) -> u16 {
        let screen = render(app, 80, 24);
        let row = screen.lines().position(|row| row.contains(marker));
        u16::try_from(row.expect("marker should be on screen")).unwrap()
    }

    #[test]
    fn results_for_an_edited_needle_are_drawn_dim() {
        use ratatui::style::Modifier;

        let mut app = searching("hello");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let row = row_with(&app, " 2  at 0x");
        let first_cell = |app: &App| cells(app, 80, 24)[(20, row)].modifier;
        assert!(!first_cell(&app).contains(Modifier::DIM));

        // Editing the needle makes the same rows stale, which is shown by dimming them.
        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(first_cell(&app).contains(Modifier::DIM));
    }

    #[test]
    fn an_opened_result_is_drawn_with_the_needle_highlighted() {
        use ratatui::style::Color;

        let mut app = searching("hello");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)); // search
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)); // open the first
        let buffer = cells(&app, 80, 24);
        let marked: String = (0..24)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|&(x, y)| buffer[(x, y)].bg == Color::Yellow)
            .map(|(x, y)| buffer[(x, y)].symbol().to_string())
            .collect();
        assert_eq!(marked, "hello");
    }
}
