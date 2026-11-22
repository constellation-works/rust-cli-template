//! Borderless tables (STD-01 R14, R15).
//!
//! One header row, exactly one line per record, two-space gutters, numbers
//! right-aligned. When a width is given (a terminal), the truncatable column
//! shrinks and an overlong cell ends in a single `…`; with no width (a pipe,
//! a file) nothing is ever cut. Width is a parameter, never read here, so
//! tests pin it. Column widths are measured in terminal display columns
//! (Unicode East Asian Wide/Fullwidth = 2, zero-width combining/control = 0).

use super::color::{Color, Role, paint};

/// The narrowest a truncatable column shrinks to.
const MIN_TRUNCATED_WIDTH: usize = 8;

/// Column alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    /// Text.
    Left,
    /// Numbers.
    Right,
}

/// A column's header and layout rules.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Column {
    /// Header text; carries the unit for numeric columns.
    pub(crate) header: &'static str,
    /// Alignment of every cell in the column.
    pub(crate) align: Align,
    /// Whether this column may be cut to fit a terminal. Only a column whose
    /// full value is available from `note show` or `--json` may be (R15).
    pub(crate) truncate: bool,
}

/// One cell: its text and what its color means.
#[derive(Clone, Debug)]
pub(crate) struct Cell {
    /// The text; an absent value is `-`, never empty (R14).
    pub(crate) text: String,
    /// Semantic color role.
    pub(crate) role: Role,
}

impl Cell {
    /// A cell with no emphasis.
    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            role: Role::Neutral,
        }
    }
}

/// Render `rows` under `columns`, each line ending in `\n`.
pub(super) fn render(
    columns: &[Column],
    rows: &[Vec<Cell>],
    width: Option<usize>,
    color: Color,
) -> String {
    let widths = fit(columns, &natural_widths(columns, rows), width);
    let mut out = String::new();
    let header: Vec<Cell> = columns.iter().map(|c| Cell::plain(c.header)).collect();
    push_line(&mut out, columns, &widths, &header, Color::Plain);
    for row in rows {
        push_line(&mut out, columns, &widths, row, color);
    }
    out
}

/// Display width of `s` in terminal columns.
pub(crate) fn display_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Terminal column width of one Unicode scalar value: 0 for control, format
/// and combining marks, 2 for East Asian Wide/Fullwidth characters, else 1.
fn char_width(c: char) -> usize {
    if c.is_control() || is_zero_width(c) {
        0
    } else if is_wide(c) {
        2
    } else {
        1
    }
}

fn is_zero_width(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036F   // Combining Diacritical Marks
        | 0x1AB0..=0x1AFF // Combining Diacritical Marks Extended
        | 0x1DC0..=0x1DFF // Combining Diacritical Marks Supplement
        | 0x200B..=0x200F // Zero-width spaces & directional marks
        | 0x202A..=0x202E // Bidirectional formatting
        | 0x2060..=0x206F // Word joiner & invisible operators
        | 0x20D0..=0x20FF // Combining Diacritical Marks for Symbols
        | 0xFE00..=0xFE0F // Variation Selectors
        | 0xFE20..=0xFE2F // Combining Half Marks
        | 0xFEFF          // Byte Order Mark / ZWNBSP
        | 0xE0100..=0xE01EF // Variation Selectors Supplement
    )
}

fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x115F   // Hangul Jamo init. consonants
        | 0x231A..=0x231B // Watch, Hourglass
        | 0x2329..=0x232A // Angle brackets
        | 0x23E9..=0x23EC // Media arrows
        | 0x23F0 | 0x23F3
        | 0x25FD..=0x25FE
        | 0x2614..=0x2615
        | 0x2648..=0x2653
        | 0x267F | 0x2693 | 0x26A1
        | 0x26AA..=0x26AB
        | 0x26BD..=0x26BE
        | 0x26C4..=0x26C5
        | 0x26CE | 0x26D4 | 0x26EA
        | 0x26F2..=0x26F3
        | 0x26F5 | 0x26FA | 0x26FD
        | 0x2705
        | 0x270A..=0x270B
        | 0x2728 | 0x274C | 0x274E
        | 0x2753..=0x2755
        | 0x2757
        | 0x2795..=0x2797
        | 0x27B0 | 0x27BF
        | 0x2B1B..=0x2B1C
        | 0x2B50 | 0x2B55
        | 0x2E80..=0x303E // CJK Radicals .. Ideographic Symbols
        | 0x3041..=0x33FF // Hiragana, Katakana, Bopomofo, CJK Compat
        | 0x3400..=0x4DBF // CJK Unified Ideographs Extension A
        | 0x4E00..=0xA4CF // CJK Unified Ideographs & Yi
        | 0xA960..=0xA97C // Hangul Jamo Extended-A
        | 0xAC00..=0xD7A3 // Hangul Syllables
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0xFE10..=0xFE19 // Vertical Forms
        | 0xFE30..=0xFE6F // CJK Compatibility Forms
        | 0xFF01..=0xFF60 // Fullwidth Forms
        | 0xFFE0..=0xFFE6 // Fullwidth Symbol Variants
        | 0x16FE0..=0x16FE4
        | 0x16FF0..=0x16FF1
        | 0x17000..=0x187F7
        | 0x18800..=0x18CD5
        | 0x18D00..=0x18D08
        | 0x1AFF0..=0x1AFFE
        | 0x1B000..=0x1B122
        | 0x1B132
        | 0x1B150..=0x1B152
        | 0x1B155
        | 0x1B164..=0x1B167
        | 0x1B170..=0x1B2FB
        | 0x1F004 | 0x1F0CF | 0x1F18E
        | 0x1F191..=0x1F19A
        | 0x1F200..=0x1F2FF
        | 0x1F300..=0x1F64F
        | 0x1F680..=0x1F6FF
        | 0x1F900..=0x1F9FF
        | 0x1FA70..=0x1FAFF
        | 0x20000..=0x2FFFD // CJK Extensions B..F
        | 0x30000..=0x3FFFD // CJK Extensions G..I
    )
}

fn natural_widths(columns: &[Column], rows: &[Vec<Cell>]) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            rows.iter()
                .filter_map(|r| r.get(i))
                .map(|cell| display_width(&cell.text))
                .chain([display_width(col.header)])
                .max()
                .unwrap_or(0)
        })
        .collect()
}

/// Shrink truncatable columns until the line fits `width`, if it can.
fn fit(columns: &[Column], natural: &[usize], width: Option<usize>) -> Vec<usize> {
    let mut widths = natural.to_vec();
    let Some(limit) = width else {
        return widths;
    };
    let gutters = 2 * columns.len().saturating_sub(1);
    let mut excess = (widths.iter().sum::<usize>() + gutters).saturating_sub(limit);
    for (i, col) in columns.iter().enumerate() {
        if excess == 0 {
            break;
        }
        if col.truncate {
            let floor = MIN_TRUNCATED_WIDTH.max(display_width(col.header));
            let give = excess.min(widths[i].saturating_sub(floor));
            widths[i] -= give;
            excess -= give;
        }
    }
    widths
}

fn push_line(out: &mut String, columns: &[Column], widths: &[usize], cells: &[Cell], color: Color) {
    let last = columns.len().saturating_sub(1);
    for (i, (col, &w)) in columns.iter().zip(widths).enumerate() {
        let (text, role) = cells
            .get(i)
            .map_or(("-", Role::Neutral), |c| (c.text.as_str(), c.role));
        let text = truncate(text, w);
        let pad = " ".repeat(w.saturating_sub(display_width(&text)));
        let painted = paint(&text, role, color);
        match col.align {
            Align::Right => {
                out.push_str(&pad);
                out.push_str(&painted);
            }
            Align::Left if i == last => out.push_str(&painted),
            Align::Left => {
                out.push_str(&painted);
                out.push_str(&pad);
            }
        }
        if i != last {
            out.push_str("  ");
        }
    }
    out.truncate(out.trim_end_matches(' ').len());
    out.push('\n');
}

/// Cut `text` to `width` display columns, ending in one `…` when anything was cut.
pub(crate) fn truncate(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let keep = width.saturating_sub(1);
    let mut cut = String::new();
    let mut used = 0usize;
    for c in text.chars() {
        let cw = char_width(c);
        if used + cw > keep {
            break;
        }
        cut.push(c);
        used += cw;
    }
    cut.push('…');
    cut
}
