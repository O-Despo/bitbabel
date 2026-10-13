//! The key map for the explore screen. One table gives each key its [`Action`], and the hint
//! bar is generated from the same table, so they cannot drift apart.
//!
//! Text typed into a prompt bypasses the table: typing `l` into a path box never means "next
//! page".

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What a key does while exploring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NextPage,
    PrevPage,
    HistoryBack,
    HistoryForward,
    Random,
    GoTo,
    Search,
    NextResult,
    PrevResult,
    Export,
    CycleView,
    Quit,
    Help,
    StartOver,
    ScrollDown,
    ScrollUp,
    HalfPageDown,
    HalfPageUp,
    Top,
    Bottom,
}

/// A key as the table writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pattern {
    /// A character, with Shift ignored: the character already carries the case.
    Char(char),
    /// Ctrl and a letter.
    Ctrl(char),
    /// Any other key.
    Code(KeyCode),
}

/// One row of the table.
struct Binding {
    pattern: Pattern,
    action: Action,
    /// The text this key adds to the hint bar, if it is shown there.
    hint: Option<&'static str>,
    /// What the key does, for the help screen. A key with `None` is not listed there.
    help: Option<&'static str>,
}

const fn bind(
    pattern: Pattern,
    action: Action,
    hint: Option<&'static str>,
    help: Option<&'static str>,
) -> Binding {
    Binding {
        pattern,
        action,
        hint,
        help,
    }
}

/// Every key, in the order the hint bar shows them.
const BINDINGS: &[Binding] = &[
    bind(
        Pattern::Char('h'),
        Action::PrevPage,
        Some("h/l page"),
        Some("previous page"),
    ),
    bind(
        Pattern::Char('l'),
        Action::NextPage,
        None,
        Some("next page"),
    ),
    bind(
        Pattern::Char('j'),
        Action::ScrollDown,
        Some("j/k scroll"),
        Some("scroll down one line"),
    ),
    bind(
        Pattern::Char('k'),
        Action::ScrollUp,
        None,
        Some("scroll up one line"),
    ),
    bind(
        Pattern::Ctrl('d'),
        Action::HalfPageDown,
        None,
        Some("scroll down half a screen"),
    ),
    bind(
        Pattern::Ctrl('u'),
        Action::HalfPageUp,
        None,
        Some("scroll up half a screen"),
    ),
    bind(
        Pattern::Code(KeyCode::Home),
        Action::Top,
        None,
        Some("top of the page"),
    ),
    bind(
        Pattern::Code(KeyCode::End),
        Action::Bottom,
        None,
        Some("bottom of the page"),
    ),
    bind(
        Pattern::Code(KeyCode::Left),
        Action::HistoryBack,
        None,
        Some("back in history"),
    ),
    bind(
        Pattern::Code(KeyCode::Right),
        Action::HistoryForward,
        None,
        Some("forward in history"),
    ),
    bind(
        Pattern::Char('r'),
        Action::Random,
        Some("r random"),
        Some("a random page"),
    ),
    bind(
        Pattern::Char('g'),
        Action::GoTo,
        Some("g go to"),
        Some("go to an index from a file"),
    ),
    bind(
        Pattern::Char('/'),
        Action::Search,
        Some("/ search"),
        Some("search for text"),
    ),
    bind(
        Pattern::Char('n'),
        Action::NextResult,
        None,
        Some("next search result"),
    ),
    bind(
        Pattern::Char('N'),
        Action::PrevResult,
        None,
        Some("previous search result"),
    ),
    bind(
        Pattern::Char('x'),
        Action::Export,
        None,
        Some("save the index to a new file"),
    ),
    bind(
        Pattern::Char('e'),
        Action::CycleView,
        Some("e view"),
        Some("cycle text, hex, base64, utf8"),
    ),
    bind(
        Pattern::Char('s'),
        Action::StartOver,
        None,
        Some("start over, another library"),
    ),
    bind(
        Pattern::Char('q'),
        Action::Quit,
        Some("q quit"),
        Some("quit (Ctrl+c also quits)"),
    ),
    bind(
        Pattern::Char('?'),
        Action::Help,
        Some("? help"),
        Some("this help"),
    ),
    bind(Pattern::Ctrl('c'), Action::Quit, None, None),
];

/// The action for `key`, if it has one.
///
/// Shift is ignored. Ctrl alone looks up a Ctrl shortcut. Alt alone does nothing. Ctrl and Alt
/// together is how Windows reports AltGr (`@`, `~`, `\`), so it is typing, not a command.
pub fn action_for(key: &KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let pattern = match (ctrl, alt, key.code) {
        (false, false, KeyCode::Char(c)) => Pattern::Char(c),
        (true, false, KeyCode::Char(c)) => Pattern::Ctrl(c.to_ascii_lowercase()),
        (false, false, code) => Pattern::Code(code),
        _ => return None,
    };
    BINDINGS
        .iter()
        .find(|binding| binding.pattern == pattern)
        .map(|binding| binding.action)
}

/// Whether `key` is Ctrl+C, which quits from every mode.
pub fn is_interrupt(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL
}

impl Pattern {
    /// How the key is written on the help screen: `h`, `Ctrl+d`, `Home`.
    fn label(self) -> String {
        match self {
            Pattern::Char(c) => c.to_string(),
            Pattern::Ctrl(c) => format!("Ctrl+{c}"),
            Pattern::Code(KeyCode::Left) => "←".to_string(),
            Pattern::Code(KeyCode::Right) => "→".to_string(),
            Pattern::Code(code) => format!("{code}"),
        }
    }
}

/// Every key the help screen lists, with what it does, in table order.
pub fn help_entries() -> Vec<(String, &'static str)> {
    BINDINGS
        .iter()
        .filter_map(|binding| Some((binding.pattern.label(), binding.help?)))
        .collect()
}

/// The hint bar text, from the table.
pub fn hints() -> String {
    let parts: Vec<&str> = BINDINGS.iter().filter_map(|b| b.hint).collect();
    parts.join("  ")
}

#[cfg(test)]
mod test {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    fn plain(c: char) -> Option<Action> {
        action_for(&key(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn plain_keys_find_their_action() {
        assert_eq!(plain('h'), Some(Action::PrevPage));
        assert_eq!(plain('l'), Some(Action::NextPage));
        assert_eq!(plain('e'), Some(Action::CycleView));
        assert_eq!(plain('r'), Some(Action::Random));
        assert_eq!(plain('x'), Some(Action::Export));
        assert_eq!(plain('g'), Some(Action::GoTo));
        assert_eq!(plain('/'), Some(Action::Search));
        assert_eq!(plain('n'), Some(Action::NextResult));
        assert_eq!(plain('N'), Some(Action::PrevResult));
        assert_eq!(
            action_for(&key(KeyCode::Left, KeyModifiers::NONE)),
            Some(Action::HistoryBack)
        );
        assert_eq!(
            action_for(&key(KeyCode::Right, KeyModifiers::NONE)),
            Some(Action::HistoryForward)
        );
        assert_eq!(plain('j'), Some(Action::ScrollDown));
        assert_eq!(plain('k'), Some(Action::ScrollUp));
        assert_eq!(plain('q'), Some(Action::Quit));
        assert_eq!(plain('z'), None);
        assert_eq!(
            action_for(&key(KeyCode::Home, KeyModifiers::NONE)),
            Some(Action::Top)
        );
    }

    #[test]
    fn shift_is_ignored_because_the_character_carries_the_case() {
        // Shift+j still scrolls, and Shift+q is `Q`, which is not bound.
        assert_eq!(
            action_for(&key(KeyCode::Char('j'), KeyModifiers::SHIFT)),
            Some(Action::ScrollDown)
        );
        assert_eq!(
            action_for(&key(KeyCode::Char('Q'), KeyModifiers::SHIFT)),
            None
        );
    }

    #[test]
    fn ctrl_alone_is_a_shortcut() {
        let ctrl = |c| action_for(&key(KeyCode::Char(c), KeyModifiers::CONTROL));
        assert_eq!(ctrl('d'), Some(Action::HalfPageDown));
        assert_eq!(ctrl('u'), Some(Action::HalfPageUp));
        assert_eq!(ctrl('c'), Some(Action::Quit));
        // Plain `d` is not Ctrl+D, and Ctrl+J is not plain `j`.
        assert_eq!(plain('d'), None);
        assert_eq!(ctrl('j'), None);
    }

    #[test]
    fn alt_alone_does_nothing() {
        assert_eq!(
            action_for(&key(KeyCode::Char('j'), KeyModifiers::ALT)),
            None
        );
    }

    #[test]
    fn ctrl_with_alt_is_typing_not_a_command() {
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(action_for(&key(KeyCode::Char('j'), altgr)), None);
        assert_eq!(action_for(&key(KeyCode::Char('c'), altgr)), None);
        assert!(!is_interrupt(&key(KeyCode::Char('c'), altgr)));
    }

    #[test]
    fn only_plain_ctrl_c_interrupts() {
        assert!(is_interrupt(&key(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_interrupt(&key(KeyCode::Char('c'), KeyModifiers::NONE)));
    }

    #[test]
    fn question_mark_is_help() {
        assert_eq!(plain('?'), Some(Action::Help));
    }

    #[test]
    fn the_help_entries_come_from_the_table() {
        let entries = help_entries();
        let find = |key: &str| {
            entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, what)| *what)
        };
        assert_eq!(find("h"), Some("previous page"));
        assert_eq!(find("Ctrl+d"), Some("scroll down half a screen"));
        assert_eq!(find("Home"), Some("top of the page"));
        assert_eq!(find("←"), Some("back in history"));
        assert_eq!(find("?"), Some("this help"));
        // Ctrl+c is the same as `q` and has no line of its own.
        assert_eq!(find("Ctrl+c"), None);
    }

    #[test]
    fn every_key_but_ctrl_c_is_on_the_help_screen() {
        let listed = help_entries().len();
        assert_eq!(listed, BINDINGS.len() - 1);
    }

    #[test]
    fn no_two_bindings_share_a_key() {
        for (at, binding) in BINDINGS.iter().enumerate() {
            for other in &BINDINGS[at + 1..] {
                assert_ne!(binding.pattern, other.pattern);
            }
        }
    }

    #[test]
    fn the_hint_bar_comes_from_the_table_and_fits_80_columns() {
        let hints = hints();
        assert_eq!(
            hints,
            "h/l page  j/k scroll  r random  g go to  / search  e view  q quit  ? help"
        );
        assert!(hints.chars().count() <= 78);
    }
}
