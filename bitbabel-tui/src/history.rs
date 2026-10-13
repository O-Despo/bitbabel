//! The pages visited so far, like a browser's history.

use std::collections::VecDeque;

use bitbabel_core::PageIndex;

/// The most pages remembered. Older ones are forgotten first.
const CAP: usize = 100;

/// Visited pages with a position in them. Going back and then visiting a new page drops the
/// pages that were ahead.
#[derive(Debug, Clone)]
pub struct History {
    entries: VecDeque<PageIndex>,
    /// Which entry is the current page.
    at: usize,
}

impl History {
    /// A history that starts at `first`.
    pub fn new(first: PageIndex) -> Self {
        History {
            entries: VecDeque::from([first]),
            at: 0,
        }
    }

    /// The page the history is at.
    pub fn current(&self) -> &PageIndex {
        &self.entries[self.at]
    }

    /// Records a visit. Visiting the page you are already on adds nothing; only consecutive
    /// repeats are skipped, so `A, B, A` is three entries.
    pub fn visit(&mut self, index: PageIndex) {
        if *self.current() == index {
            return;
        }
        self.entries.truncate(self.at + 1);
        self.entries.push_back(index);
        if self.entries.len() > CAP {
            self.entries.pop_front();
        }
        self.at = self.entries.len() - 1;
    }

    /// Moves back one page and returns it, or `None` at the start.
    pub fn back(&mut self) -> Option<&PageIndex> {
        self.at = self.at.checked_sub(1)?;
        Some(&self.entries[self.at])
    }

    /// Moves forward one page and returns it, or `None` at the end.
    pub fn forward(&mut self) -> Option<&PageIndex> {
        if self.at + 1 >= self.entries.len() {
            return None;
        }
        self.at += 1;
        Some(&self.entries[self.at])
    }

    /// How many pages are remembered.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn page(n: u64) -> PageIndex {
        PageIndex::from_u64(n, 4)
    }

    /// The remembered pages as numbers.
    fn pages(history: &History) -> Vec<u32> {
        history
            .entries
            .iter()
            .map(|index| u32::from_be_bytes(index.as_bytes().try_into().unwrap()))
            .collect()
    }

    #[test]
    fn the_plan_example() {
        // start A; l, l -> A+1, A+2; r -> R; g X
        let (a, r, x) = (100, 7, 55);
        let mut history = History::new(page(a));
        history.visit(page(a + 1));
        history.visit(page(a + 2));
        history.visit(page(r));
        history.visit(page(x));
        assert_eq!(pages(&history), [100, 101, 102, 7, 55]);

        assert_eq!(history.back(), Some(&page(r)));
        assert_eq!(history.back(), Some(&page(a + 2)));
        assert_eq!(history.forward(), Some(&page(r)));

        // Back to R's neighbour A+2, then a new visit drops R and X.
        history.back();
        history.visit(page(9));
        assert_eq!(pages(&history), [100, 101, 102, 9]);
    }

    #[test]
    fn back_and_forward_stop_at_the_ends() {
        let mut history = History::new(page(1));
        assert_eq!(history.back(), None);
        assert_eq!(history.forward(), None);
        history.visit(page(2));
        assert_eq!(history.back(), Some(&page(1)));
        assert_eq!(history.back(), None);
        assert_eq!(history.current(), &page(1));
        assert_eq!(history.forward(), Some(&page(2)));
        assert_eq!(history.forward(), None);
    }

    #[test]
    fn only_consecutive_duplicates_are_skipped() {
        let mut history = History::new(page(1));
        history.visit(page(1));
        assert_eq!(history.len(), 1);
        history.visit(page(2));
        history.visit(page(1));
        assert_eq!(pages(&history), [1, 2, 1]);
    }

    #[test]
    fn a_duplicate_of_the_current_page_keeps_the_forward_entries() {
        let mut history = History::new(page(1));
        history.visit(page(2));
        history.back();
        history.visit(page(1));
        assert_eq!(pages(&history), [1, 2]);
        assert_eq!(history.forward(), Some(&page(2)));
    }

    #[test]
    fn it_remembers_at_most_100_pages() {
        let mut history = History::new(page(0));
        for n in 1..=150 {
            history.visit(page(n));
        }
        assert_eq!(history.len(), 100);
        assert_eq!(history.current(), &page(150));
        // The oldest were pushed out: the earliest left is 51.
        assert_eq!(pages(&history)[0], 51);
        for _ in 0..99 {
            assert!(history.back().is_some());
        }
        assert_eq!(history.back(), None);
        assert_eq!(history.current(), &page(51));
    }
}
