//! Health badge helpers.
//!
//! # Design
//! - Map health status strings to badge classes without side effects.
//! - Invariants: unknown statuses always render a neutral badge.
//! - Failure modes: none; unrecognized values fall back to ghost styling.

use yew::{Classes, classes};

/// Build a status badge class list for health state.
#[must_use]
pub(crate) fn status_badge(status: &str) -> Classes {
    let tone = match status {
        "ok" | "healthy" | "active" => Some("badge-success"),
        "warn" | "warning" | "degraded" => Some("badge-warning"),
        "error" | "failed" => Some("badge-error"),
        _ => None,
    };
    let mut classes = classes!("badge", "badge-sm");
    if let Some(tone) = tone {
        classes.push(tone);
        classes.push("badge-soft");
    } else {
        classes.push("badge-ghost");
    }
    classes
}

#[cfg(test)]
mod tests {
    use super::status_badge;

    #[test]
    fn status_badge_marks_known_statuses() {
        assert!(status_badge("ok").contains("badge-success"));
        assert!(status_badge("warning").contains("badge-warning"));
        assert!(status_badge("failed").contains("badge-error"));
        assert!(status_badge("unknown").contains("badge-ghost"));
    }
}
