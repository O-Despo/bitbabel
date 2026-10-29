//! The Explore screen's state: one open library, the page shown and how it is viewed. All of
//! its rules live here, so they run in plain unit tests without a browser.

use std::rc::Rc;

use bitbabel_core::PageIndex;
use bitbabel_explorer::{DisplayLine, LibraryChoice, View, render};
use yew::Reducible;

/// How many characters of an index or hexagon are kept at each end when it is shortened.
const ENDS: usize = 12;

/// A message under the command row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    pub error: bool,
}

/// An open library at one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explore {
    library: LibraryChoice,
    index: PageIndex,
    view: View,
    /// The bytes of the page at `index`.
    bytes: Vec<u8>,
    status: Option<Status>,
}

impl Explore {
    /// Opens `library` at the page whose index is `seed`. A fresh random `seed` of the page
    /// length gives a random page.
    ///
    /// # Errors
    ///
    /// A message if the library cannot be opened or `seed` is not one index long.
    pub fn open(library: LibraryChoice, seed: Vec<u8>) -> Result<Self, String> {
        let index = PageIndex::from_bytes(seed);
        let bytes = page_bytes(&library, &index)?;
        Ok(Explore {
            library,
            index,
            view: View::Text,
            bytes,
            status: None,
        })
    }

    /// The open library.
    pub fn library(&self) -> &LibraryChoice {
        &self.library
    }

    /// The index of the page shown.
    #[cfg(test)]
    pub fn index(&self) -> &PageIndex {
        &self.index
    }

    /// How the page is shown.
    pub fn view(&self) -> View {
        self.view
    }

    /// The bytes of the page shown.
    #[cfg(test)]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The message under the command row, if any.
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    /// The page laid out in the current view.
    pub fn lines(&self) -> Vec<DisplayLine> {
        render(self.view, &self.bytes, None)
    }

    /// The whole index, as base64.
    pub fn index_text(&self) -> String {
        self.index.to_base64()
    }

    /// The index with the middle left out: `AAAAAAAAAAAA…AAAAAAAAAAAA`.
    pub fn index_short(&self) -> String {
        shorten(&self.index_text())
    }

    /// The page's address, with a long hexagon shortened.
    pub fn location_text(&self) -> String {
        let location = self.index.location();
        format!(
            "hexagon {} · wall {} · shelf {} · volume {}",
            shorten(&location.hexagon_hex()),
            location.wall(),
            location.shelf(),
            location.volume()
        )
    }

    /// The name and contents of the file that saves the index: `index-AAAAAAAA.idx`.
    pub fn index_file(&self) -> (String, String) {
        let text = self.index_text();
        // `/` and `+` are not safe in a file name.
        let start: String = text
            .chars()
            .take(8)
            .map(|c| match c {
                '/' => '_',
                '+' => '-',
                c => c,
            })
            .collect();
        (format!("index-{start}.idx"), text)
    }

    /// Goes to the next page or the one before it, wrapping at the ends.
    pub fn step(&mut self, forward: bool) {
        let mut index = self.index.clone();
        if forward {
            index.increment();
        } else {
            index.decrement();
        }
        self.show(index);
    }

    /// Goes to a random page, from `seed` bytes. If there are none, says why and stays.
    pub fn random(&mut self, seed: Result<Vec<u8>, String>) {
        match seed {
            Ok(seed) => self.show(PageIndex::from_bytes(seed)),
            Err(error) => self.say(format!("cannot get random bytes: {error}"), true),
        }
    }

    /// The index written as base64 in `text`, for a page of this library.
    ///
    /// # Errors
    ///
    /// A message for the screen if `text` is not base64 or not one index long.
    pub fn parse_index(&self, text: &str) -> Result<PageIndex, String> {
        PageIndex::from_base64(text.trim(), self.index.as_bytes().len())
            .map_err(|error| error.to_string())
    }

    /// Goes to the page at `index`.
    pub fn visit(&mut self, index: PageIndex) {
        self.show(index);
    }

    /// Shows the same page in another view.
    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    /// Sets the message under the command row.
    pub fn say(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(Status {
            text: text.into(),
            error,
        });
    }

    /// Shows the page at `index`. If it can't be computed, says why and changes nothing else.
    fn show(&mut self, index: PageIndex) {
        match page_bytes(&self.library, &index) {
            Ok(bytes) => {
                self.index = index;
                self.bytes = bytes;
                self.status = None;
            }
            Err(error) => self.say(error, true),
        }
    }
}

/// The page at `index` in `library`.
fn page_bytes(library: &LibraryChoice, index: &PageIndex) -> Result<Vec<u8>, String> {
    let opened = library.open().map_err(|error| error.to_string())?;
    let page = opened.page_at(index).map_err(|error| error.to_string())?;
    Ok(page.bytes().to_vec())
}

/// `text` with the middle replaced by `…` when it is longer than both ends and the dots.
fn shorten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= ENDS * 2 + 1 {
        return text.to_string();
    }
    let head: String = chars[..ENDS].iter().collect();
    let tail: String = chars[chars.len() - ENDS..].iter().collect();
    format!("{head}…{tail}")
}

/// What the Explore screen holds: nothing until a library is opened.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Slot {
    #[default]
    Empty,
    Open(Box<Explore>),
    /// Opening failed. The text says why.
    Failed(String),
}

/// A change to the Explore screen.
#[derive(Debug)]
pub enum ExploreAction {
    /// Open a library at the page `seed` names, dropping whatever was open.
    Open {
        library: LibraryChoice,
        seed: Result<Vec<u8>, String>,
    },
    Step(bool),
    Random(Result<Vec<u8>, String>),
    Visit(PageIndex),
    View(View),
    Say {
        text: String,
        error: bool,
    },
}

impl Reducible for Slot {
    type Action = ExploreAction;

    fn reduce(self: Rc<Self>, action: ExploreAction) -> Rc<Self> {
        if let ExploreAction::Open { library, seed } = action {
            let opened = seed.and_then(|seed| Explore::open(library, seed));
            return Rc::new(match opened {
                Ok(explore) => Slot::Open(Box::new(explore)),
                Err(error) => Slot::Failed(error),
            });
        }
        let mut next = (*self).clone();
        if let Slot::Open(explore) = &mut next {
            match action {
                ExploreAction::Open { .. } => {}
                ExploreAction::Step(forward) => explore.step(forward),
                ExploreAction::Random(seed) => explore.random(seed),
                ExploreAction::Visit(index) => explore.visit(index),
                ExploreAction::View(view) => explore.set_view(view),
                ExploreAction::Say { text, error } => explore.say(text, error),
            }
        }
        Rc::new(next)
    }
}

#[cfg(test)]
mod test {
    use bitbabel_core::{BabelLibrary, LibraryConfig};

    use super::*;

    fn small() -> LibraryChoice {
        LibraryChoice::new(LibraryConfig::SMALL, None)
    }

    fn number(value: u64) -> PageIndex {
        PageIndex::from_u64(value, 16)
    }

    fn at_zero() -> Explore {
        Explore::open(small(), vec![0; 16]).unwrap()
    }

    #[test]
    fn open_shows_the_page_at_the_seed() {
        let explore = at_zero();
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        let expected = library.page_at(&number(0)).unwrap();
        assert_eq!(explore.index(), &number(0));
        assert_eq!(explore.bytes(), expected.bytes());
        assert_eq!(explore.view(), View::Text);
        assert!(explore.status().is_none());
    }

    #[test]
    fn a_seed_of_the_wrong_length_is_an_error() {
        assert!(Explore::open(small(), vec![0; 3]).is_err());
    }

    #[test]
    fn step_moves_by_one_and_wraps_at_both_ends() {
        let mut explore = at_zero();
        explore.step(true);
        assert_eq!(explore.index(), &number(1));
        explore.step(false);
        explore.step(false);
        assert_eq!(explore.index(), &PageIndex::from_bytes(vec![0xff; 16]));
        explore.step(true);
        assert_eq!(explore.index(), &number(0));
    }

    #[test]
    fn step_shows_that_pages_bytes() {
        let mut explore = at_zero();
        let before = explore.bytes().to_vec();
        explore.step(true);
        let library = BabelLibrary::canonical(LibraryConfig::SMALL).unwrap();
        assert_eq!(
            explore.bytes(),
            library.page_at(&number(1)).unwrap().bytes()
        );
        assert_ne!(explore.bytes(), before);
    }

    #[test]
    fn random_goes_to_the_seed_and_a_failed_source_changes_nothing() {
        let mut explore = at_zero();
        explore.random(Ok(vec![7; 16]));
        assert_eq!(explore.index(), &PageIndex::from_bytes(vec![7; 16]));

        explore.random(Err("blocked".to_string()));
        assert_eq!(explore.index(), &PageIndex::from_bytes(vec![7; 16]));
        assert_eq!(
            explore.status(),
            Some(&Status {
                text: "cannot get random bytes: blocked".to_string(),
                error: true
            })
        );
    }

    #[test]
    fn an_index_is_read_from_base64_and_visited() {
        let mut explore = at_zero();
        let index = explore
            .parse_index(&format!(" {}\n", number(5).to_base64()))
            .unwrap();
        assert_eq!(index, number(5));
        explore.visit(index);
        assert_eq!(explore.index(), &number(5));
    }

    #[test]
    fn a_bad_index_is_an_error_message() {
        let explore = at_zero();
        assert_eq!(
            explore.parse_index("AAAA"),
            Err("index must be 16 bytes, got 3".to_string())
        );
        assert!(
            explore
                .parse_index("not base64!")
                .unwrap_err()
                .starts_with("invalid index")
        );
    }

    #[test]
    fn the_saved_index_file_reads_back_to_the_same_page() {
        let mut explore = Explore::open(small(), vec![9; 16]).unwrap();
        let (_, contents) = explore.index_file();
        explore.step(true);
        let index = explore.parse_index(&contents).unwrap();
        explore.visit(index);
        assert_eq!(explore.index(), &PageIndex::from_bytes(vec![9; 16]));
    }

    #[test]
    fn a_page_change_clears_the_status() {
        let mut explore = at_zero();
        explore.say("copied", false);
        assert!(explore.status().is_some());
        explore.step(true);
        assert!(explore.status().is_none());
    }

    #[test]
    fn changing_the_view_keeps_the_page() {
        let mut explore = at_zero();
        let bytes = explore.bytes().to_vec();
        let text_lines = explore.lines();
        explore.set_view(View::Hex);
        assert_eq!(explore.bytes(), bytes);
        assert_ne!(explore.lines(), text_lines);
    }

    #[test]
    fn the_address_has_a_wall_shelf_and_volume() {
        let explore = at_zero();
        assert!(
            explore
                .location_text()
                .ends_with("wall 1 · shelf 1 · volume 1")
        );
    }

    #[test]
    fn a_long_index_is_shortened_at_both_ends() {
        assert_eq!(shorten("short"), "short");
        let long = "abcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(shorten(long), "abcdefghijkl…yz0123456789");
    }

    #[test]
    fn the_saved_index_has_a_safe_name_and_the_whole_index() {
        let explore = Explore::open(small(), vec![0xff; 16]).unwrap();
        let (name, contents) = explore.index_file();
        assert_eq!(name, "index-________.idx");
        assert_eq!(contents, explore.index_text());
        assert_eq!(contents.len(), 24);
    }

    #[test]
    fn the_slot_ignores_actions_until_a_library_is_open() {
        let empty = Rc::new(Slot::Empty).reduce(ExploreAction::Step(true));
        assert_eq!(*empty, Slot::Empty);

        let open = empty.reduce(ExploreAction::Open {
            library: small(),
            seed: Ok(vec![0; 16]),
        });
        let Slot::Open(explore) = &*open else {
            panic!("expected an open library");
        };
        assert_eq!(explore.index(), &number(0));

        let stepped = open.reduce(ExploreAction::Step(true));
        let Slot::Open(explore) = &*stepped else {
            panic!("expected an open library");
        };
        assert_eq!(explore.index(), &number(1));
    }

    #[test]
    fn opening_with_a_failed_seed_is_a_failed_slot() {
        let slot = Rc::new(Slot::Empty).reduce(ExploreAction::Open {
            library: small(),
            seed: Err("no random".to_string()),
        });
        assert_eq!(*slot, Slot::Failed("no random".to_string()));
    }
}
