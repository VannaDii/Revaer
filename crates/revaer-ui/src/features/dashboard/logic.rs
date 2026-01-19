//! Dashboard-specific helpers for formatting and icon selection.
//!
//! # Design
//! - Keep formatting and asset selection pure and deterministic.
//! - Invariants: icon selection cycles within bounded asset sets.
//! - Failure modes: division-by-zero yields safe defaults.

use crate::models::EventKind;

/// Badge metadata for recent events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EventBadge {
    pub(crate) class: &'static str,
    pub(crate) label_key: &'static str,
}

/// Choose the avatar for queue summaries.
#[must_use]
pub(crate) fn queue_icon_src(index: usize) -> String {
    let avatar = (index % 5) + 1;
    format!("/static/nexus/images/avatars/{avatar}.png")
}

/// Choose the product image for recent events.
#[must_use]
pub(crate) fn event_icon_src(index: usize) -> String {
    let product = (index % 10) + 1;
    format!("/static/nexus/images/apps/ecommerce/products/{product}.jpg")
}

/// Format capacity in GB or TB.
#[must_use]
pub(crate) fn format_capacity(gb: u32) -> String {
    if gb >= 1024 {
        let tb = f64::from(gb) / 1024.0;
        format!("{tb:.1} TB")
    } else {
        format!("{gb} GB")
    }
}

/// Compute usage percentage for disk usage.
#[must_use]
pub(crate) fn usage_percent(used: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        f64::from(used) / f64::from(total) * 100.0
    }
}

/// Map event kind to badge styling and translation key.
#[must_use]
pub(crate) const fn event_badge(kind: EventKind) -> EventBadge {
    match kind {
        EventKind::Info => EventBadge {
            class: "badge-info",
            label_key: "dashboard.event_info",
        },
        EventKind::Warning => EventBadge {
            class: "badge-warning",
            label_key: "dashboard.event_warn",
        },
        EventKind::Error => EventBadge {
            class: "badge-error",
            label_key: "dashboard.event_error",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_capacity_switches_to_tb() {
        assert_eq!(format_capacity(256), "256 GB");
        assert_eq!(format_capacity(2048), "2.0 TB");
    }

    #[test]
    fn usage_percent_handles_zero_total() {
        assert!((usage_percent(10, 0) - 0.0).abs() < f64::EPSILON);
        assert!((usage_percent(25, 100) - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn icon_sources_rotate() {
        assert!(queue_icon_src(0).contains("avatars/1.png"));
        assert!(event_icon_src(0).contains("products/1.jpg"));
    }

    #[test]
    fn event_badge_maps_kind() {
        assert_eq!(event_badge(EventKind::Info).class, "badge-info");
        assert_eq!(
            event_badge(EventKind::Error).label_key,
            "dashboard.event_error"
        );
    }
}
