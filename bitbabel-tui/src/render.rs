//! Page bytes to display lines: the bytes are split into glyphs, and the glyphs are broken
//! into lines by byte count, so a glyph is never split across lines.

use std::ops::Range;

use bitbabel_core::{BabelGuaranteedText, Base64, Glyph, Glyphs, Hex, Utf8};

/// How page bytes are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// One visible character per byte, from the BitBabel text table.
    Text,
    /// Two hex digits per byte.
    Hex,
    /// Standard padded base64.
    Base64,
    /// UTF-8, with `·` for bytes and characters that cannot be shown.
    Utf8,
}

impl View {
    /// The next view in the cycle: text, hex, base64, utf8, then text again.
    // Used by the view key.
    #[allow(dead_code)]
    pub const fn next(self) -> Self {
        match self {
            View::Text => View::Hex,
            View::Hex => View::Base64,
            View::Base64 => View::Utf8,
            View::Utf8 => View::Text,
        }
    }

    /// The name shown in the title bar.
    pub const fn name(self) -> &'static str {
        match self {
            View::Text => "text",
            View::Hex => "hex",
            View::Base64 => "base64",
            View::Utf8 => "utf8",
        }
    }

    /// The most bytes on one line. Each view then draws at most 64 characters per line:
    /// 64 text characters, 32 bytes of hex, 48 bytes of base64 (a multiple of 3, so padding
    /// only ever comes at the end of the page), or up to 64 UTF-8 bytes.
    pub const fn line_bytes(self) -> usize {
        match self {
            View::Text | View::Utf8 => 64,
            View::Hex => 32,
            View::Base64 => 48,
        }
    }

    fn glyphs(self, bytes: &[u8]) -> Vec<Glyph> {
        match self {
            View::Text => BabelGuaranteedText::glyphs(bytes),
            View::Hex => Hex::glyphs(bytes),
            View::Base64 => Base64::glyphs(bytes),
            View::Utf8 => Utf8::glyphs(bytes),
        }
    }
}

/// A run of text on a line that is either all highlighted or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub highlighted: bool,
}

/// One row of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLine {
    /// The offset of the line's first byte in the page, shown in the gutter.
    pub offset: usize,
    pub segments: Vec<Segment>,
}

impl DisplayLine {
    /// The line's text without the highlight marks.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }
}

/// Lays `bytes` out in `view`. Every glyph whose bytes overlap `highlight` is marked, so a
/// match that starts inside a multi-byte character marks the whole character.
pub fn render(view: View, bytes: &[u8], highlight: Option<&Range<usize>>) -> Vec<DisplayLine> {
    let capacity = view.line_bytes();
    let mut lines: Vec<DisplayLine> = Vec::new();
    // The line being built, and the byte it started at.
    let mut current: Option<DisplayLine> = None;
    let mut line_start = 0;

    for glyph in view.glyphs(bytes) {
        let range = glyph.bytes();

        // If this glyph would make the line longer than its capacity, finish the line.
        if let Some(full) = current.take_if(|_| range.end - line_start > capacity) {
            lines.push(full);
        }
        if current.is_none() {
            line_start = range.start;
            current = Some(DisplayLine {
                offset: range.start,
                segments: Vec::new(),
            });
        }

        let highlighted = highlight.is_some_and(|h| h.start < range.end && range.start < h.end);
        if let Some(line) = current.as_mut() {
            match line.segments.last_mut() {
                // Same highlight as the run before it: extend that run.
                Some(last) if last.highlighted == highlighted => last.text.push_str(glyph.text()),
                _ => line.segments.push(Segment {
                    text: glyph.text().to_string(),
                    highlighted,
                }),
            }
        }
    }
    if let Some(last) = current {
        lines.push(last);
    }
    lines
}

#[cfg(test)]
mod test {
    use super::*;

    fn bytes(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 37 % 256) as u8).collect()
    }

    fn offsets(lines: &[DisplayLine]) -> Vec<usize> {
        lines.iter().map(|line| line.offset).collect()
    }

    #[test]
    fn the_view_cycle_visits_every_view_and_returns() {
        let mut view = View::Text;
        let mut seen = vec![view];
        for _ in 0..4 {
            view = view.next();
            seen.push(view);
        }
        assert_eq!(
            seen,
            [View::Text, View::Hex, View::Base64, View::Utf8, View::Text]
        );
    }

    #[test]
    fn line_counts_match_the_plan_for_every_size() {
        let counts = |view, len| render(view, &bytes(len), None).len();
        assert_eq!(counts(View::Text, 16), 1);
        assert_eq!(counts(View::Text, 3200), 50);
        assert_eq!(counts(View::Text, 6400), 100);
        assert_eq!(counts(View::Hex, 16), 1);
        assert_eq!(counts(View::Hex, 3200), 100);
        assert_eq!(counts(View::Hex, 6400), 200);
        assert_eq!(counts(View::Base64, 16), 1);
        assert_eq!(counts(View::Base64, 3200), 67);
        assert_eq!(counts(View::Base64, 6400), 134);
    }

    #[test]
    fn gutter_steps_match_the_plan() {
        let data = bytes(3200);
        assert_eq!(
            offsets(&render(View::Text, &data, None))[..3],
            [0, 0x40, 0x80]
        );
        assert_eq!(
            offsets(&render(View::Hex, &data, None))[..3],
            [0, 0x20, 0x40]
        );
        assert_eq!(
            offsets(&render(View::Base64, &data, None))[..3],
            [0, 0x30, 0x60]
        );
    }

    #[test]
    fn full_lines_are_exactly_64_characters_and_none_is_wider() {
        let data = bytes(3200);
        for view in [View::Text, View::Hex, View::Base64] {
            let lines = render(view, &data, None);
            let (last, full) = lines.split_last().unwrap();
            assert!(full.iter().all(|line| line.text().chars().count() == 64));
            assert!(last.text().chars().count() <= 64);
        }
    }

    #[test]
    fn base64_only_pads_at_the_end_of_the_page() {
        let lines = render(View::Base64, &bytes(3200), None);
        let (last, rest) = lines.split_last().unwrap();
        assert!(rest.iter().all(|line| !line.text().contains('=')));
        // 3200 = 66 * 48 + 32, so the last line is 32 bytes: 44 characters, 10 + 2 padding.
        assert!(last.text().ends_with('='));
    }

    #[test]
    fn text_lines_hold_the_table_characters() {
        let line = &render(View::Text, b"Hello", None)[0];
        assert_eq!(line.text(), "Hello");
        assert_eq!(line.offset, 0);
    }

    #[test]
    fn utf8_moves_a_character_that_does_not_fit_to_the_next_line() {
        // 63 ASCII bytes, then a 3-byte character at byte 63.
        let mut data = vec![b'a'; 63];
        data.extend("中".as_bytes());
        let lines = render(View::Utf8, &data, None);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "a".repeat(63));
        assert_eq!(lines[1].text(), "中");
        assert_eq!(lines[1].offset, 0x3f);
    }

    #[test]
    fn utf8_lines_never_exceed_64_bytes() {
        let data = "中".repeat(100).into_bytes();
        for line in render(View::Utf8, &data, None) {
            assert!(line.text().len() <= 64);
        }
    }

    #[test]
    fn every_byte_is_on_exactly_one_line() {
        for view in [View::Text, View::Hex, View::Base64, View::Utf8] {
            let data = bytes(1000);
            let lines = render(view, &data, None);
            assert_eq!(lines[0].offset, 0, "{view:?}");
            let offsets = offsets(&lines);
            assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]), "{view:?}");
        }
    }

    #[test]
    fn empty_bytes_give_no_lines() {
        assert!(render(View::Text, &[], None).is_empty());
    }

    #[test]
    fn highlight_marks_every_overlapping_glyph() {
        // "aé" is bytes 61 c3 a9. A match on byte 1 (inside the é) marks all of é.
        let lines = render(View::Utf8, "aé!".as_bytes(), Some(&(1..2)));
        let segments = &lines[0].segments;
        assert_eq!(
            segments,
            &[
                Segment {
                    text: "a".to_string(),
                    highlighted: false
                },
                Segment {
                    text: "é".to_string(),
                    highlighted: true
                },
                Segment {
                    text: "!".to_string(),
                    highlighted: false
                },
            ]
        );
    }

    #[test]
    fn highlight_runs_across_a_line_break() {
        let data = vec![b'a'; 70];
        let lines = render(View::Text, &data, Some(&(60..66)));
        assert!(lines[0].segments.last().unwrap().highlighted);
        assert!(lines[1].segments[0].highlighted);
        assert_eq!(lines[1].segments[0].text.chars().count(), 2);
    }

    #[test]
    fn a_match_inside_a_base64_group_marks_the_group() {
        let lines = render(View::Base64, &[1, 2, 3, 4, 5, 6], Some(&(4..5)));
        let marked: Vec<&str> = lines[0]
            .segments
            .iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(marked, ["BAUG"]);
    }
}
