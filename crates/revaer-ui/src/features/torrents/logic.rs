//! Torrent view helpers extracted for testing.
//!
//! # Design
//! - Isolate sort, parse, and label logic for deterministic unit tests.
//! - Invariants: sorting is stable per state; parsing is non-panicking.
//! - Failure modes: missing rows or invalid inputs yield safe defaults.

use std::cmp::Ordering;

use crate::core::store::AppStore;
use crate::core::ui::Pane;
use crate::features::torrents::actions::TorrentAction;
use crate::features::torrents::state::{
    FsopsBadge, FsopsStatus, TorrentRow, TorrentSortDirection, TorrentSortKey, TorrentSortState,
};
use crate::models::{FilePriority, TorrentStateKind};
use uuid::Uuid;

/// Determine the next sort state for a column toggle.
#[must_use]
pub(crate) fn next_sort_state(
    current: Option<TorrentSortState>,
    key: TorrentSortKey,
) -> Option<TorrentSortState> {
    match current {
        None => Some(TorrentSortState {
            key,
            direction: TorrentSortDirection::Asc,
        }),
        Some(state) if state.key != key => Some(TorrentSortState {
            key,
            direction: TorrentSortDirection::Asc,
        }),
        Some(state) => match state.direction {
            TorrentSortDirection::Asc => Some(TorrentSortState {
                key,
                direction: TorrentSortDirection::Desc,
            }),
            TorrentSortDirection::Desc => None,
        },
    }
}

/// Return the sort indicator attribute for a column header.
#[must_use]
pub(crate) fn sort_indicator(
    sort_state: Option<TorrentSortState>,
    key: TorrentSortKey,
) -> &'static str {
    match sort_state {
        Some(state) if state.key == key => match state.direction {
            TorrentSortDirection::Asc => "asc",
            TorrentSortDirection::Desc => "desc",
        },
        _ => "none",
    }
}

/// Sort torrent IDs by the requested state.
#[must_use]
pub(crate) fn sort_ids(
    ids: &[Uuid],
    store: &AppStore,
    sort_state: Option<TorrentSortState>,
) -> Vec<Uuid> {
    let mut next: Vec<Uuid> = ids.to_vec();
    let Some(sort_state) = sort_state else {
        return next;
    };
    next.sort_by(|a, b| compare_row(store, a, b, sort_state));
    next
}

fn compare_row(
    store: &AppStore,
    left: &Uuid,
    right: &Uuid,
    sort_state: TorrentSortState,
) -> Ordering {
    let Some(left_row) = store.torrents.by_id.get(left) else {
        return Ordering::Equal;
    };
    let Some(right_row) = store.torrents.by_id.get(right) else {
        return Ordering::Equal;
    };
    let order = match sort_state.key {
        TorrentSortKey::Name => left_row.name.cmp(&right_row.name),
        TorrentSortKey::State => left_row.status.cmp(&right_row.status),
        TorrentSortKey::Progress => cmp_f64(left_row.progress, right_row.progress),
        TorrentSortKey::Down => left_row.download_bps.cmp(&right_row.download_bps),
        TorrentSortKey::Up => left_row.upload_bps.cmp(&right_row.upload_bps),
        TorrentSortKey::Ratio => cmp_f64(left_row.ratio, right_row.ratio),
        TorrentSortKey::Size => left_row.size_bytes.cmp(&right_row.size_bytes),
        TorrentSortKey::Eta => {
            eta_value(left_row.eta.as_deref()).cmp(&eta_value(right_row.eta.as_deref()))
        }
        TorrentSortKey::Tags => first_tag(left_row).cmp(first_tag(right_row)),
        TorrentSortKey::Trackers => left_row.tracker.cmp(&right_row.tracker),
        TorrentSortKey::Updated => left_row.updated.cmp(&right_row.updated),
    };
    match sort_state.direction {
        TorrentSortDirection::Asc => order,
        TorrentSortDirection::Desc => order.reverse(),
    }
}

fn cmp_f64(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

#[must_use]
pub(crate) fn eta_value(eta: Option<&str>) -> u64 {
    eta.and_then(|value| value.trim_end_matches('s').parse::<u64>().ok())
        .unwrap_or(u64::MAX)
}

#[must_use]
pub(crate) fn first_tag(row: &TorrentRow) -> &str {
    row.tags.first().map_or("", String::as_str)
}

/// Status badge classes for torrent rows.
#[must_use]
pub(crate) fn status_badge_class(status: &str) -> &'static str {
    match status {
        "downloading" | "seeding" | "completed" => "badge-success",
        "fetching_metadata" | "queued" => "badge-info",
        "checking" => "badge-warning",
        "error" | "failed" => "badge-error",
        _ => "badge-neutral",
    }
}

/// FS ops badge classes for row labels.
#[must_use]
pub(crate) fn fsops_badge_class(fsops: Option<&FsopsBadge>) -> &'static str {
    match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => "badge-warning",
        Some(FsopsStatus::Completed) => "badge-success",
        Some(FsopsStatus::Failed) => "badge-error",
        None => "badge-neutral",
    }
}

/// Translation keys for FS ops badge labels.
#[must_use]
pub(crate) fn fsops_label_key(fsops: Option<&FsopsBadge>) -> Option<&'static str> {
    match fsops.map(|badge| &badge.status) {
        Some(FsopsStatus::InProgress) => Some("torrents.fsops_in_progress"),
        Some(FsopsStatus::Completed) => Some("torrents.fsops_done"),
        Some(FsopsStatus::Failed) => Some("torrents.fsops_failed"),
        None => None,
    }
}

/// Translation key for action banner messages.
#[must_use]
pub(crate) const fn action_banner_label_key(action: &TorrentAction) -> &'static str {
    match action {
        TorrentAction::Delete { with_data: true } => "torrents.banner.removed_data",
        TorrentAction::Delete { with_data: false } => "torrents.banner.removed",
        TorrentAction::Reannounce => "torrents.banner.reannounce",
        TorrentAction::Recheck => "torrents.banner.recheck",
        TorrentAction::Pause => "torrents.banner.pause",
        TorrentAction::Resume => "torrents.banner.resume",
        TorrentAction::Sequential { enable } => {
            if *enable {
                "torrents.banner.sequential_on"
            } else {
                "torrents.banner.sequential_off"
            }
        }
        TorrentAction::Rate { .. } => "torrents.banner.rate",
    }
}

/// Check if a DOM target should block row click handling.
#[must_use]
pub(crate) fn is_interactive_tag(tag: &str, role: Option<&str>) -> bool {
    if let Some(role) = role
        && role.eq_ignore_ascii_case("button")
    {
        return true;
    }
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "button" | "a" | "input" | "select" | "textarea" | "label" | "summary"
    )
}

/// Pane visibility class for the detail drawer.
#[must_use]
pub(crate) fn pane_visibility_class(pane: Pane, active: Pane) -> &'static str {
    if pane == active { "block" } else { "hidden" }
}

/// Display label for torrent state.
#[must_use]
pub(crate) const fn state_label(state: TorrentStateKind) -> &'static str {
    match state {
        TorrentStateKind::Queued => "queued",
        TorrentStateKind::FetchingMetadata => "fetching_metadata",
        TorrentStateKind::Downloading => "downloading",
        TorrentStateKind::Seeding => "seeding",
        TorrentStateKind::Completed => "completed",
        TorrentStateKind::Failed => "failed",
        TorrentStateKind::Stopped => "stopped",
    }
}

/// Status class for detail header badges.
#[must_use]
pub(crate) const fn status_class(state: TorrentStateKind) -> &'static str {
    match state {
        TorrentStateKind::Downloading | TorrentStateKind::Seeding | TorrentStateKind::Completed => {
            "badge-success"
        }
        TorrentStateKind::Failed => "badge-error",
        TorrentStateKind::FetchingMetadata => "badge-warning",
        _ => "badge-ghost",
    }
}

/// Renderable file priority token.
#[must_use]
pub(crate) const fn priority_value(priority: FilePriority) -> &'static str {
    match priority {
        FilePriority::Skip => "skip",
        FilePriority::Low => "low",
        FilePriority::Normal => "normal",
        FilePriority::High => "high",
    }
}

/// Error returned when parsing a priority token fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsePriorityError {
    pub(crate) value: String,
}

/// Parse a priority token into a [`FilePriority`].
pub(crate) fn parse_priority(value: &str) -> Result<FilePriority, ParsePriorityError> {
    match value {
        "skip" => Ok(FilePriority::Skip),
        "low" => Ok(FilePriority::Low),
        "normal" => Ok(FilePriority::Normal),
        "high" => Ok(FilePriority::High),
        _ => Err(ParsePriorityError {
            value: value.to_string(),
        }),
    }
}

/// Parse an optional integer input.
pub(crate) fn parse_optional_i32(value: &str) -> Result<Option<i32>, std::num::ParseIntError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some)
}

/// Parse a comma-separated list into trimmed values.
#[must_use]
pub(crate) fn parse_list(value: &str) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect()
}

/// Return None when the string is empty.
#[must_use]
pub(crate) fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::torrents::state::{FsopsBadge, FsopsStatus};
    use crate::models::TorrentStateKind;
    use std::rc::Rc;

    fn sample_row(id: Uuid, name: &str, eta: Option<&str>, tags: Vec<String>) -> TorrentRow {
        TorrentRow {
            id,
            name: name.to_string(),
            status: "downloading".to_string(),
            progress: 0.5,
            eta: eta.map(str::to_string),
            ratio: 1.0,
            updated: "now".to_string(),
            tags,
            tracker: String::new(),
            path: String::new(),
            category: String::new(),
            size_bytes: 1024,
            upload_bps: 0,
            download_bps: 0,
        }
    }

    #[test]
    fn next_sort_state_cycles() {
        let state = next_sort_state(None, TorrentSortKey::Name);
        assert_eq!(
            state,
            Some(TorrentSortState {
                key: TorrentSortKey::Name,
                direction: TorrentSortDirection::Asc
            })
        );
        let state = next_sort_state(state, TorrentSortKey::Name);
        assert_eq!(
            state,
            Some(TorrentSortState {
                key: TorrentSortKey::Name,
                direction: TorrentSortDirection::Desc
            })
        );
        let state = next_sort_state(state, TorrentSortKey::Name);
        assert!(state.is_none());
    }

    #[test]
    fn status_badge_class_maps_states() {
        assert_eq!(status_badge_class("downloading"), "badge-success");
        assert_eq!(status_badge_class("paused"), "badge-neutral");
    }

    #[test]
    fn fsops_label_keys_map() {
        let badge = FsopsBadge {
            status: FsopsStatus::Failed,
            detail: None,
        };
        assert_eq!(fsops_label_key(Some(&badge)), Some("torrents.fsops_failed"));
    }

    #[test]
    fn state_labels_are_stable() {
        assert_eq!(state_label(TorrentStateKind::Queued), "queued");
        assert_eq!(status_class(TorrentStateKind::Failed), "badge-error");
    }

    #[test]
    fn priority_round_trip() {
        assert_eq!(parse_priority("high"), Ok(FilePriority::High));
        assert_eq!(priority_value(FilePriority::Low), "low");
        assert!(parse_priority("unknown").is_err());
    }

    #[test]
    fn sort_indicator_and_sort_ids_order() {
        let sort_state = TorrentSortState {
            key: TorrentSortKey::Name,
            direction: TorrentSortDirection::Asc,
        };
        assert_eq!(
            sort_indicator(Some(sort_state), TorrentSortKey::Name),
            "asc"
        );
        assert_eq!(
            sort_indicator(Some(sort_state), TorrentSortKey::Down),
            "none"
        );

        let mut store = AppStore::default();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        store
            .torrents
            .by_id
            .insert(id_a, Rc::new(sample_row(id_a, "Zulu", None, vec![])));
        store
            .torrents
            .by_id
            .insert(id_b, Rc::new(sample_row(id_b, "Alpha", None, vec![])));

        let sorted = sort_ids(&[id_a, id_b], &store, Some(sort_state));
        assert_eq!(sorted, vec![id_b, id_a]);
    }

    #[test]
    fn eta_and_tag_helpers_return_defaults() {
        assert_eq!(eta_value(Some("15s")), 15);
        assert_eq!(eta_value(Some("oops")), u64::MAX);
        let row = sample_row(Uuid::new_v4(), "demo", None, vec!["tag".to_string()]);
        assert_eq!(first_tag(&row), "tag");
    }

    #[test]
    fn fsops_badge_class_maps_status() {
        let badge = FsopsBadge {
            status: FsopsStatus::Completed,
            detail: None,
        };
        assert_eq!(fsops_badge_class(Some(&badge)), "badge-success");
        assert_eq!(fsops_badge_class(None), "badge-neutral");
    }

    #[test]
    fn pane_visibility_class_switches() {
        assert_eq!(
            pane_visibility_class(Pane::Overview, Pane::Overview),
            "block"
        );
        assert_eq!(pane_visibility_class(Pane::Files, Pane::Overview), "hidden");
    }

    #[test]
    fn parse_optional_i32_handles_empty_and_invalid() {
        assert_eq!(parse_optional_i32(""), Ok(None));
        assert!(parse_optional_i32("bad").is_err());
        assert_eq!(parse_optional_i32("12"), Ok(Some(12)));
    }

    #[test]
    fn parse_list_trims_and_filters() {
        assert_eq!(parse_list("a, b, ,c"), vec!["a", "b", "c"]);
        assert_eq!(optional_string(" "), None);
    }

    #[test]
    fn interactive_tag_accepts_role() {
        assert!(is_interactive_tag("div", Some("button")));
        assert!(is_interactive_tag("button", None));
        assert!(!is_interactive_tag("div", None));
    }

    #[test]
    fn action_banner_label_key_tracks_variants() {
        assert_eq!(
            action_banner_label_key(&TorrentAction::Delete { with_data: true }),
            "torrents.banner.removed_data"
        );
        assert_eq!(
            action_banner_label_key(&TorrentAction::Sequential { enable: false }),
            "torrents.banner.sequential_off"
        );
    }
}
