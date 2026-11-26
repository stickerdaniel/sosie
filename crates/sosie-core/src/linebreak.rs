// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use icu_segmenter::{
    GraphemeClusterSegmenter, GraphemeClusterSegmenterBorrowed, LineSegmenter,
    LineSegmenterBorrowed,
};
use std::sync::OnceLock;

fn line_segmenter() -> &'static LineSegmenterBorrowed<'static> {
    static S: OnceLock<LineSegmenterBorrowed<'static>> = OnceLock::new();
    S.get_or_init(|| {
        let mut options = icu_segmenter::options::LineBreakOptions::default();
        options.strictness = Some(icu_segmenter::options::LineBreakStrictness::Normal);
        LineSegmenter::new_auto(options)
    })
}

fn grapheme_segmenter() -> &'static GraphemeClusterSegmenterBorrowed<'static> {
    static S: OnceLock<GraphemeClusterSegmenterBorrowed<'static>> = OnceLock::new();
    S.get_or_init(GraphemeClusterSegmenter::new)
}

fn ascii_break(bytes: &[u8], i: usize) -> bool {
    match (bytes[i - 1], bytes[i]) {
        (b'-', d) if d.is_ascii_digit() => i >= 2 && bytes[i - 2].is_ascii_alphanumeric(),
        (a, b) => crate::blink_ascii::may_break(a, b),
    }
}

pub fn line_breaks(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if table_breaks(text, &mut out) {
        return out;
    }
    out.clear();
    general_breaks(text, &mut out);
    out
}

fn table_breaks(text: &str, out: &mut Vec<usize>) -> bool {
    use crate::blink_ascii::{FIRST, LAST};
    let bytes = text.as_bytes();
    let in_table = |b: u8| (FIRST..=LAST).contains(&b);
    let mut i = 1;
    while i < bytes.len() {
        let (a, b) = (bytes[i - 1], bytes[i]);
        if a == b' ' || a == b'\t' {
            if b != b' ' && b != b'\t' && text.is_char_boundary(i) {
                out.push(i);
            }
        } else if in_table(a) && in_table(b) {
            if ascii_break(bytes, i) {
                out.push(i);
            }
        } else if in_table(a) && (b == b' ' || b == b'\t') {
        } else {
            return false;
        }
        i += 1;
    }
    true
}

fn general_breaks(text: &str, out: &mut Vec<usize>) {
    use crate::blink_ascii::{FIRST, LAST};
    let len = text.len();
    let bytes = text.as_bytes();
    let in_table = |b: u8| (FIRST..=LAST).contains(&b);
    let mut icu = line_segmenter()
        .segment_str(text)
        .filter(|&i| i > 0 && i < len)
        .peekable();
    let mut i = 1;
    while i < len {
        let (a, b) = (bytes[i - 1], bytes[i]);
        if a == b' ' || a == b'\t' {
            if b != b' ' && b != b'\t' && text.is_char_boundary(i) {
                out.push(i);
            }
        } else if in_table(a) && in_table(b) {
            if ascii_break(bytes, i) {
                out.push(i);
            }
        } else {
            while icu.peek().is_some_and(|&p| p < i) {
                icu.next();
            }
            if icu.peek() == Some(&i) {
                out.push(i);
            }
        }
        i += 1;
    }
}

pub fn grapheme_boundaries(text: &str) -> Vec<usize> {
    if text.is_ascii() {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len() + 1);
        for i in 0..=bytes.len() {
            if i > 0 && i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n' {
                continue;
            }
            out.push(i);
        }
        return out;
    }
    grapheme_segmenter().segment_str(text).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pass_agrees_with_the_general_pass() {
        let cases = [
            "",
            "a",
            "we shipped the thing today, and it is fast.",
            "https://vercel.com/blog/the-thing",
            "a  b\tc",
            "hyphen-1234 and 12-34",
            "tab\tstop\there",
            "trailing spaces   ",
            "a\u{0}b",
            "caf\u{e9} au lait",
            "\u{1F600} emoji",
        ];
        for text in cases {
            let mut table = Vec::new();
            let decided = table_breaks(text, &mut table);
            let general = {
                let mut out = Vec::new();
                general_breaks(text, &mut out);
                out
            };
            if decided {
                assert_eq!(table, general, "disagreement on {text:?}");
            }
            assert_eq!(
                line_breaks(text),
                general,
                "line_breaks disagrees on {text:?}"
            );
        }
    }

    #[test]
    fn ascii_text_stays_on_the_table() {
        let cases = [
            "the quick brown fox jumps over the lazy dog",
            "we shipped the thing today, and it is fast.",
            "a b c d e f g h i j k l m n o p q r s t u v w x y z",
            "one  two   three    four",
            "   leading spaces",
            "trailing spaces   ",
            "\tindented with a tab",
            "col\ta\tb\tc",
            "https://vercel.com/blog/the-thing?x=1&y=2",
            "https://example.com/a-really-long-path/that/keeps/going",
            "hyphen-1234 12-34 -- --- A-1 ABC-9",
            "x: y; z, w. v? u! t",
            "!?:;,.*+%&()[]{}<>|~^_=#$@`'\"",
            "a\rb",
            "a\u{0}b",
        ];
        for text in cases {
            let mut table = Vec::new();
            let decided = table_breaks(text, &mut table);
            let general = {
                let mut out = Vec::new();
                general_breaks(text, &mut out);
                out
            };
            let ascii = text.is_ascii();
            let control = text.bytes().any(|b| b < 0x20 && b != b'\t');
            assert_eq!(
                decided,
                ascii && !control,
                "unexpected decision on {text:?}"
            );
            if decided {
                assert_eq!(table, general, "disagreement on {text:?}");
            }
            assert_eq!(
                line_breaks(text),
                general,
                "line_breaks disagrees on {text:?}"
            );
        }
    }
}
