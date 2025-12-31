//! ANSI log parsing helpers.
//!
//! # Design
//! - Parse ANSI SGR color/style codes into styled spans for rendering.
//! - Keep parsing allocation-light and resilient to malformed sequences.
//! - Preserve Unicode characters by operating on `char` boundaries.

use std::mem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnsiStyle {
    pub(crate) fg: Option<AnsiColor>,
    pub(crate) bg: Option<AnsiColor>,
    pub(crate) bold: bool,
    pub(crate) dim: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) inverse: bool,
}

impl AnsiStyle {
    fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn resolved_colors(self) -> (Option<AnsiColor>, Option<AnsiColor>) {
        if self.inverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnsiColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl AnsiColor {
    pub(crate) fn css_var(self) -> &'static str {
        match self {
            AnsiColor::Black => "--log-ansi-black",
            AnsiColor::Red => "--log-ansi-red",
            AnsiColor::Green => "--log-ansi-green",
            AnsiColor::Yellow => "--log-ansi-yellow",
            AnsiColor::Blue => "--log-ansi-blue",
            AnsiColor::Magenta => "--log-ansi-magenta",
            AnsiColor::Cyan => "--log-ansi-cyan",
            AnsiColor::White => "--log-ansi-white",
            AnsiColor::BrightBlack => "--log-ansi-bright-black",
            AnsiColor::BrightRed => "--log-ansi-bright-red",
            AnsiColor::BrightGreen => "--log-ansi-bright-green",
            AnsiColor::BrightYellow => "--log-ansi-bright-yellow",
            AnsiColor::BrightBlue => "--log-ansi-bright-blue",
            AnsiColor::BrightMagenta => "--log-ansi-bright-magenta",
            AnsiColor::BrightCyan => "--log-ansi-bright-cyan",
            AnsiColor::BrightWhite => "--log-ansi-bright-white",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnsiSpan {
    pub(crate) text: String,
    pub(crate) style: AnsiStyle,
}

pub(crate) fn parse_ansi_line(line: &str) -> Vec<AnsiSpan> {
    let mut spans = Vec::new();
    let mut style = AnsiStyle::default();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                let mut buffer = String::new();
                let mut terminated = false;

                while let Some(code) = chars.next() {
                    if code == 'm' {
                        terminated = true;
                        break;
                    }
                    buffer.push(code);
                }

                if terminated {
                    if !current.is_empty() {
                        let text = mem::take(&mut current);
                        spans.push(AnsiSpan { text, style });
                    }
                    let codes = parse_codes(&buffer);
                    apply_sgr_codes(&mut style, &codes);
                    continue;
                }

                current.push('\u{1b}');
                current.push('[');
                current.push_str(&buffer);
                break;
            }
        }

        current.push(ch);
    }

    if !current.is_empty() {
        spans.push(AnsiSpan {
            text: current,
            style,
        });
    }

    spans
}

fn parse_codes(buffer: &str) -> Vec<i32> {
    if buffer.is_empty() {
        return vec![0];
    }
    buffer
        .split(';')
        .filter_map(|part| part.parse::<i32>().ok())
        .collect()
}

fn apply_sgr_codes(style: &mut AnsiStyle, codes: &[i32]) {
    let mut index = 0usize;
    while index < codes.len() {
        let code = codes[index];
        match code {
            0 => style.reset(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            7 => style.inverse = true,
            22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            27 => style.inverse = false,
            39 => style.fg = None,
            49 => style.bg = None,
            30..=37 => style.fg = map_basic_color(code - 30, false),
            90..=97 => style.fg = map_basic_color(code - 90, true),
            40..=47 => style.bg = map_basic_color(code - 40, false),
            100..=107 => style.bg = map_basic_color(code - 100, true),
            38 | 48 => {
                index = index.saturating_add(skip_extended_color(codes, index));
            }
            _ => {}
        }
        index = index.saturating_add(1);
    }
}

fn skip_extended_color(codes: &[i32], index: usize) -> usize {
    let Some(mode) = codes.get(index.saturating_add(1)) else {
        return 0;
    };
    match *mode {
        5 => 2,
        2 => 4,
        _ => 0,
    }
}

fn map_basic_color(code: i32, bright: bool) -> Option<AnsiColor> {
    match (code, bright) {
        (0, false) => Some(AnsiColor::Black),
        (1, false) => Some(AnsiColor::Red),
        (2, false) => Some(AnsiColor::Green),
        (3, false) => Some(AnsiColor::Yellow),
        (4, false) => Some(AnsiColor::Blue),
        (5, false) => Some(AnsiColor::Magenta),
        (6, false) => Some(AnsiColor::Cyan),
        (7, false) => Some(AnsiColor::White),
        (0, true) => Some(AnsiColor::BrightBlack),
        (1, true) => Some(AnsiColor::BrightRed),
        (2, true) => Some(AnsiColor::BrightGreen),
        (3, true) => Some(AnsiColor::BrightYellow),
        (4, true) => Some(AnsiColor::BrightBlue),
        (5, true) => Some(AnsiColor::BrightMagenta),
        (6, true) => Some(AnsiColor::BrightCyan),
        (7, true) => Some(AnsiColor::BrightWhite),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{AnsiColor, AnsiStyle, parse_ansi_line};

    #[test]
    fn parse_plain_line_retains_text() {
        let spans = parse_ansi_line("plain log line");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "plain log line");
        assert_eq!(spans[0].style, AnsiStyle::default());
    }

    #[test]
    fn parse_color_codes_split_spans() {
        let spans = parse_ansi_line("alpha\u{1b}[31mred\u{1b}[0momega");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "alpha");
        assert_eq!(spans[1].text, "red");
        assert_eq!(spans[1].style.fg, Some(AnsiColor::Red));
        assert_eq!(spans[2].text, "omega");
        assert_eq!(spans[2].style, AnsiStyle::default());
    }

    #[test]
    fn parse_style_flags_are_applied() {
        let spans = parse_ansi_line("\u{1b}[1;4mstrong\u{1b}[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].style.bold);
        assert!(spans[0].style.underline);
    }
}
