//! Pure log processing helpers for the logs view.
//!
//! # Design
//! - Keep parsing/filtering deterministic and side-effect free for tests.
//! - Invariants: log window uses monotonic timestamps; filters are inclusive by level.
//! - Failure modes: malformed lines fall back to `Unknown` level without panics.

use std::collections::VecDeque;

use crate::features::logs::ansi::{AnsiSpan, parse_ansi_line};

/// Rolling window length for log retention.
pub(crate) const LOG_WINDOW_MS: u64 = 120_000;

/// Parsed log severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

impl LogLevel {
    const fn severity(self) -> u8 {
        match self {
            Self::Trace => 0,
            Self::Debug => 1,
            Self::Info | Self::Unknown => 2,
            Self::Warn => 3,
            Self::Error => 4,
        }
    }
}

/// User-selected log filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogLevelFilter {
    All,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    pub(crate) const fn as_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    #[must_use]
    pub(crate) fn from_value(value: &str) -> Self {
        match value {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::All,
        }
    }

    const fn min_severity(self) -> u8 {
        match self {
            Self::All | Self::Trace => 0,
            Self::Debug => 1,
            Self::Info => 2,
            Self::Warn => 3,
            Self::Error => 4,
        }
    }

    /// Returns true when the log level meets or exceeds the filter.
    #[must_use]
    pub(crate) const fn matches(self, level: LogLevel) -> bool {
        level.severity() >= self.min_severity()
    }
}

/// Parsed log line ready for rendering.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LogLine {
    pub(crate) spans: Vec<AnsiSpan>,
    pub(crate) plain_lower: String,
    pub(crate) level: LogLevel,
    pub(crate) received_at_ms: u64,
}

/// Build a log line from raw input; returns None for empty lines.
#[must_use]
pub(crate) fn build_log_line(raw: &str, received_at_ms: u64) -> Option<LogLine> {
    if raw.trim().is_empty() {
        return None;
    }
    let spans = parse_ansi_line(raw);
    let plain = spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<String>();
    let level = detect_level(&plain);
    Some(LogLine {
        spans,
        plain_lower: plain.to_lowercase(),
        level,
        received_at_ms,
    })
}

/// Filter log lines by level and search term.
#[must_use]
pub(crate) fn filter_lines<'a>(
    lines: &'a VecDeque<LogLine>,
    level_filter: LogLevelFilter,
    search_term: &str,
) -> Vec<&'a LogLine> {
    let search_lower = search_term.trim().to_lowercase();
    lines
        .iter()
        .filter(|line| {
            level_filter.matches(line.level)
                && (search_lower.is_empty() || line.plain_lower.contains(&search_lower))
        })
        .collect()
}

/// Remove stale log lines outside the rolling window; returns true when lines were pruned.
pub(crate) fn prune_old_lines(lines: &mut VecDeque<LogLine>, now_ms: u64) -> bool {
    let cutoff = now_ms.saturating_sub(LOG_WINDOW_MS);
    let mut removed = false;
    while matches!(lines.back(), Some(line) if line.received_at_ms < cutoff) {
        lines.pop_back();
        removed = true;
    }
    removed
}

fn detect_level(line: &str) -> LogLevel {
    let trimmed = line.trim();
    if trimmed.starts_with('{')
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed)
        && let Some(level) = value.get("level").and_then(|value| value.as_str())
        && let Some(parsed) = level_from_token(level)
    {
        return parsed;
    }
    for token in trimmed.split_whitespace() {
        if let Some(parsed) = level_from_token(token) {
            return parsed;
        }
    }
    LogLevel::Unknown
}

fn level_from_token(token: &str) -> Option<LogLevel> {
    let trimmed = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic());
    if let Some((key, value)) = trimmed.split_once('=')
        && (key.eq_ignore_ascii_case("level") || key.eq_ignore_ascii_case("lvl"))
    {
        return parse_level_token(value);
    }
    if let Some((key, value)) = trimmed.split_once(':')
        && (key.eq_ignore_ascii_case("level") || key.eq_ignore_ascii_case("lvl"))
    {
        return parse_level_token(value);
    }
    parse_level_token(trimmed)
}

fn parse_level_token(token: &str) -> Option<LogLevel> {
    let normalized = token.trim_matches(|ch: char| !ch.is_ascii_alphabetic());
    if normalized.eq_ignore_ascii_case("trace") {
        Some(LogLevel::Trace)
    } else if normalized.eq_ignore_ascii_case("debug") {
        Some(LogLevel::Debug)
    } else if normalized.eq_ignore_ascii_case("info") {
        Some(LogLevel::Info)
    } else if normalized.eq_ignore_ascii_case("warn") || normalized.eq_ignore_ascii_case("warning")
    {
        Some(LogLevel::Warn)
    } else if normalized.eq_ignore_ascii_case("error") {
        Some(LogLevel::Error)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_log_line_skips_empty() {
        assert!(build_log_line("  ", 0).is_none());
    }

    #[test]
    fn detect_level_parses_json() {
        let line = r#"{"level":"warn","message":"ok"}"#;
        let parsed = build_log_line(line, 1).expect("log line");
        assert_eq!(parsed.level, LogLevel::Warn);
    }

    #[test]
    fn detect_level_parses_tokens() {
        let parsed = build_log_line("level=error failure", 1).expect("log line");
        assert_eq!(parsed.level, LogLevel::Error);
        let parsed = build_log_line("WARN something", 1).expect("log line");
        assert_eq!(parsed.level, LogLevel::Warn);
    }

    #[test]
    fn filter_lines_applies_level_and_search() {
        let mut lines = VecDeque::new();
        let line = build_log_line("INFO alpha", 1).expect("log line");
        let error = build_log_line("ERROR beta", 1).expect("log line");
        lines.push_front(line);
        lines.push_front(error);

        let filtered = filter_lines(&lines, LogLevelFilter::Warn, "");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].level, LogLevel::Error);

        let filtered = filter_lines(&lines, LogLevelFilter::All, "alpha");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn log_level_filter_round_trips_and_matches() {
        let entries = [
            (LogLevelFilter::All, "all"),
            (LogLevelFilter::Trace, "trace"),
            (LogLevelFilter::Debug, "debug"),
            (LogLevelFilter::Info, "info"),
            (LogLevelFilter::Warn, "warn"),
            (LogLevelFilter::Error, "error"),
        ];
        for (filter, value) in entries {
            assert_eq!(filter.as_value(), value);
            assert_eq!(LogLevelFilter::from_value(value), filter);
        }
        assert_eq!(LogLevelFilter::from_value("nope"), LogLevelFilter::All);
        assert!(LogLevelFilter::Error.matches(LogLevel::Error));
        assert!(!LogLevelFilter::Error.matches(LogLevel::Warn));
    }

    #[test]
    fn prune_old_lines_respects_window() {
        let mut lines = VecDeque::new();
        let old = build_log_line("INFO old", 1).expect("log line");
        let recent = build_log_line("INFO recent", LOG_WINDOW_MS + 1).expect("log line");
        lines.push_front(recent);
        lines.push_back(old);

        let removed = prune_old_lines(&mut lines, LOG_WINDOW_MS + 10);
        assert!(removed);
        assert_eq!(lines.len(), 1);
    }
}
