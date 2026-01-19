//! Connectivity indicator helpers.
//!
//! # Design
//! - Derive UI indicator state from SSE summaries without side effects.
//! - Invariants: class names remain stable; retry labels appear only with retry metadata.
//! - Failure modes: missing retry data yields empty labels instead of panics.

use crate::core::store::{SseConnectionState, SseError, SseStatusSummary};

/// Icon to render for the connectivity indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IndicatorIcon {
    /// SSE is connected.
    Connected,
    /// SSE is reconnecting.
    Reconnecting,
    /// SSE is disconnected.
    Disconnected,
}

/// Visual indicator output for connectivity status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IndicatorStyle {
    /// Icon for the status.
    pub(crate) icon: IndicatorIcon,
    /// `DaisyUI` status class.
    pub(crate) status_class: &'static str,
    /// Tooltip/title text.
    pub(crate) title: String,
}

/// Build the indicator style for the topbar status badge.
#[must_use]
pub(crate) fn indicator_style(summary: &SseStatusSummary, now_ms: u64) -> IndicatorStyle {
    let retry_label = retry_in_seconds(summary.next_retry_at_ms, now_ms)
        .map(|value| format!("Reconnecting in {value}"))
        .unwrap_or_default();
    match summary.state {
        SseConnectionState::Connected => IndicatorStyle {
            icon: IndicatorIcon::Connected,
            status_class: "status-success",
            title: "Live".to_string(),
        },
        SseConnectionState::Reconnecting => IndicatorStyle {
            icon: IndicatorIcon::Reconnecting,
            status_class: "status-warning",
            title: if retry_label.is_empty() {
                "Reconnecting".to_string()
            } else {
                retry_label
            },
        },
        SseConnectionState::Disconnected => IndicatorStyle {
            icon: IndicatorIcon::Disconnected,
            status_class: if summary.has_error {
                "status-error"
            } else {
                "status-warning"
            },
            title: "Disconnected".to_string(),
        },
    }
}

/// Calculate retry seconds remaining for display.
#[must_use]
pub(crate) fn retry_in_seconds(next_retry_at_ms: Option<u64>, now_ms: u64) -> Option<String> {
    let next_retry_at_ms = next_retry_at_ms?;
    let remaining_ms = next_retry_at_ms.saturating_sub(now_ms);
    let remaining_secs = remaining_ms.div_ceil(1000);
    Some(format!("{remaining_secs}s"))
}

/// Format the last SSE error message for display.
#[must_use]
pub(crate) fn format_error(error: &SseError) -> String {
    error.status_code.map_or_else(
        || error.message.clone(),
        |code| format!("{} ({code})", error.message),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::store::SseStatusSummary;

    #[test]
    fn indicator_style_maps_states() {
        let style = indicator_style(
            &SseStatusSummary {
                state: SseConnectionState::Connected,
                next_retry_at_ms: None,
                has_error: false,
            },
            0,
        );
        assert_eq!(style.icon, IndicatorIcon::Connected);
        assert_eq!(style.status_class, "status-success");

        let style = indicator_style(
            &SseStatusSummary {
                state: SseConnectionState::Disconnected,
                next_retry_at_ms: None,
                has_error: true,
            },
            0,
        );
        assert_eq!(style.icon, IndicatorIcon::Disconnected);
        assert_eq!(style.status_class, "status-error");
    }

    #[test]
    fn retry_in_seconds_rounds_up() {
        assert_eq!(retry_in_seconds(Some(1500), 0), Some("2s".to_string()));
        assert_eq!(retry_in_seconds(None, 0), None);
    }

    #[test]
    fn format_error_includes_status_when_present() {
        let error = SseError {
            message: "Oops".to_string(),
            status_code: Some(401),
        };
        assert_eq!(format_error(&error), "Oops (401)".to_string());
    }
}
