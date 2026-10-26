//! Which lines of a page are on screen: the first visible line and how many rows there are.

/// A window of `rows` lines over a page, starting at line `top`.
///
/// The page's line count is passed to each method that needs it, so a viewport never holds a
/// stale count when the page or the view changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    top: usize,
    rows: usize,
}

impl Viewport {
    /// A viewport of `rows` lines at the top of the page.
    pub fn new(rows: usize) -> Self {
        Viewport { top: 0, rows }
    }

    /// The first visible line.
    pub fn top(&self) -> usize {
        self.top
    }

    /// How many lines fit on screen.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The largest `top` of a page of `total` lines: the last line sits on the last row.
    fn max_top(&self, total: usize) -> usize {
        total.saturating_sub(self.rows)
    }

    /// Keeps the window inside a page of `total` lines.
    pub fn clamp(&mut self, total: usize) {
        self.top = self.top.min(self.max_top(total));
    }

    /// Changes how many lines fit on screen, and keeps the window inside the page.
    pub fn set_rows(&mut self, rows: usize, total: usize) {
        self.rows = rows;
        self.clamp(total);
    }

    /// Scrolls by `lines` (negative is up), stopping at the first and last line.
    pub fn scroll_by(&mut self, lines: isize, total: usize) {
        self.top = self.top.saturating_add_signed(lines);
        self.clamp(total);
    }

    /// Scrolls half a screen down or up.
    pub fn scroll_half_page(&mut self, down: bool, total: usize) {
        let half = (self.rows / 2).max(1) as isize;
        self.scroll_by(if down { half } else { -half }, total);
    }

    /// Scrolls to the first line.
    pub fn go_top(&mut self) {
        self.top = 0;
    }

    /// Scrolls so the last line is on the last row.
    pub fn go_bottom(&mut self, total: usize) {
        self.top = self.max_top(total);
    }

    /// Scrolls so `line` is the first visible line, or as near as the page allows.
    pub fn go_line(&mut self, line: usize, total: usize) {
        self.top = line;
        self.clamp(total);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// A page of 50 lines (a medium page in the text view) on an 18-row screen.
    const TOTAL: usize = 50;

    fn viewport() -> Viewport {
        Viewport::new(18)
    }

    #[test]
    fn scrolling_stops_at_both_ends() {
        let mut view = viewport();
        view.scroll_by(-5, TOTAL);
        assert_eq!(view.top(), 0);
        view.scroll_by(1000, TOTAL);
        assert_eq!(view.top(), 32);
        view.go_top();
        assert_eq!(view.top(), 0);
        view.go_bottom(TOTAL);
        assert_eq!(view.top(), 32);
    }

    #[test]
    fn half_a_page_is_half_the_rows() {
        let mut view = viewport();
        view.scroll_half_page(true, TOTAL);
        assert_eq!(view.top(), 9);
        view.scroll_half_page(false, TOTAL);
        assert_eq!(view.top(), 0);
    }

    #[test]
    fn a_one_row_screen_still_moves_by_a_line() {
        let mut view = Viewport::new(1);
        view.scroll_half_page(true, TOTAL);
        assert_eq!(view.top(), 1);
    }

    #[test]
    fn a_page_that_fits_never_scrolls() {
        let mut view = viewport();
        view.scroll_by(5, 1);
        assert_eq!(view.top(), 0);
        view.go_bottom(1);
        assert_eq!(view.top(), 0);
        view.go_line(7, 1);
        assert_eq!(view.top(), 0);
    }

    #[test]
    fn a_taller_screen_pulls_the_window_back_inside_the_page() {
        let mut view = viewport();
        view.go_bottom(TOTAL);
        assert_eq!(view.top(), 32);
        view.set_rows(40, TOTAL);
        assert_eq!(view.top(), 10);
        view.set_rows(100, TOTAL);
        assert_eq!(view.top(), 0);
        assert_eq!(view.rows(), 100);
    }

    #[test]
    fn to_line_puts_that_line_first_unless_the_page_ends_sooner() {
        let mut view = viewport();
        view.go_line(10, TOTAL);
        assert_eq!(view.top(), 10);
        view.go_line(49, TOTAL);
        assert_eq!(view.top(), 32);
    }

    #[test]
    fn clamp_fixes_a_window_after_the_page_got_shorter() {
        let mut view = viewport();
        view.go_bottom(TOTAL);
        view.clamp(20);
        assert_eq!(view.top(), 2);
    }
}
