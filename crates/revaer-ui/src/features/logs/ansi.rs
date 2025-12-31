//! ANSI log parsing helpers.
//!
//! # Design
//! - Parse ANSI SGR color/style codes into styled spans for rendering.
//! - Keep parsing allocation-light and resilient to malformed sequences.
//! - Preserve Unicode characters by operating on `char` boundaries.

use std::mem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StyleFlags {
    bits: u8,
}

impl StyleFlags {
    const BOLD: u8 = 1 << 0;
    const DIM: u8 = 1 << 1;
    const ITALIC: u8 = 1 << 2;
    const UNDERLINE: u8 = 1 << 3;
    const INVERSE: u8 = 1 << 4;

    const fn contains(self, flag: u8) -> bool {
        self.bits & flag != 0
    }

    const fn set(&mut self, flag: u8, enabled: bool) {
        if enabled {
            self.bits |= flag;
        } else {
            self.bits &= !flag;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AnsiStyle {
    fg: Option<AnsiColor>,
    bg: Option<AnsiColor>,
    flags: StyleFlags,
}

impl AnsiStyle {
    fn reset(&mut self) {
        *self = Self::default();
    }

    const fn has_flag(self, flag: u8) -> bool {
        self.flags.contains(flag)
    }

    const fn set_flag(&mut self, flag: u8, enabled: bool) {
        self.flags.set(flag, enabled);
    }

    pub(super) const fn is_bold(self) -> bool {
        self.has_flag(StyleFlags::BOLD)
    }

    pub(super) const fn is_dim(self) -> bool {
        self.has_flag(StyleFlags::DIM)
    }

    pub(super) const fn is_italic(self) -> bool {
        self.has_flag(StyleFlags::ITALIC)
    }

    pub(super) const fn is_underline(self) -> bool {
        self.has_flag(StyleFlags::UNDERLINE)
    }

    pub(super) const fn resolved_colors(self) -> (Option<AnsiColor>, Option<AnsiColor>) {
        if self.has_flag(StyleFlags::INVERSE) {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AnsiColor {
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
    pub(super) const fn css_var(self) -> &'static str {
        match self {
            Self::Black => "--log-ansi-black",
            Self::Red => "--log-ansi-red",
            Self::Green => "--log-ansi-green",
            Self::Yellow => "--log-ansi-yellow",
            Self::Blue => "--log-ansi-blue",
            Self::Magenta => "--log-ansi-magenta",
            Self::Cyan => "--log-ansi-cyan",
            Self::White => "--log-ansi-white",
            Self::BrightBlack => "--log-ansi-bright-black",
            Self::BrightRed => "--log-ansi-bright-red",
            Self::BrightGreen => "--log-ansi-bright-green",
            Self::BrightYellow => "--log-ansi-bright-yellow",
            Self::BrightBlue => "--log-ansi-bright-blue",
            Self::BrightMagenta => "--log-ansi-bright-magenta",
            Self::BrightCyan => "--log-ansi-bright-cyan",
            Self::BrightWhite => "--log-ansi-bright-white",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AnsiSpan {
    pub(super) text: String,
    pub(super) style: AnsiStyle,
}

pub(super) fn parse_ansi_line(line: &str) -> Vec<AnsiSpan> {
    let mut spans = Vec::new();
    let mut style = AnsiStyle::default();
    let mut current = String::new();
    let mut chars = line.chars().peekable();

    loop {
        let Some(ch) = chars.next() else {
            break;
        };

        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            let mut buffer = String::new();
            let mut terminated = false;

            loop {
                let Some(code) = chars.next() else {
                    break;
                };
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
            1 => style.set_flag(StyleFlags::BOLD, true),
            2 => style.set_flag(StyleFlags::DIM, true),
            3 => style.set_flag(StyleFlags::ITALIC, true),
            4 => style.set_flag(StyleFlags::UNDERLINE, true),
            7 => style.set_flag(StyleFlags::INVERSE, true),
            22 => {
                style.set_flag(StyleFlags::BOLD, false);
                style.set_flag(StyleFlags::DIM, false);
            }
            23 => style.set_flag(StyleFlags::ITALIC, false),
            24 => style.set_flag(StyleFlags::UNDERLINE, false),
            27 => style.set_flag(StyleFlags::INVERSE, false),
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

const fn map_basic_color(code: i32, bright: bool) -> Option<AnsiColor> {
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
        let (fg, bg) = spans[1].style.resolved_colors();
        assert_eq!(fg, Some(AnsiColor::Red));
        assert_eq!(bg, None);
        assert_eq!(spans[2].text, "omega");
        assert_eq!(spans[2].style, AnsiStyle::default());
    }

    #[test]
    fn parse_style_flags_are_applied() {
        let spans = parse_ansi_line("\u{1b}[1;2;3;4mstrong\u{1b}[0m");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].style.is_bold());
        assert!(spans[0].style.is_dim());
        assert!(spans[0].style.is_italic());
        assert!(spans[0].style.is_underline());
    }

    #[test]
    fn ansi_color_css_vars_are_stable() {
        assert_eq!(AnsiColor::Red.css_var(), "--log-ansi-red");
        assert_eq!(AnsiColor::BrightGreen.css_var(), "--log-ansi-bright-green");
    }
}
