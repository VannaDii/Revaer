//! Shared torrent models and pure state transformations for testing outside wasm.

use crate::models::{DetailData, TorrentSummary};
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use uuid::Uuid;

/// UI-friendly torrent snapshot used across list/state helpers.
#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRow {
    /// Stable torrent identifier.
    pub id: Uuid,
    /// Display name for the torrent.
    pub name: String,
    /// Torrent status string.
    pub status: String,
    /// Completion percentage in the range 0.0–1.0.
    pub progress: f32,
    /// Human-readable ETA or en dash when unknown.
    pub eta: Option<String>,
    /// Current ratio for the torrent.
    pub ratio: f32,
    /// Any labels applied to the torrent.
    pub tags: Vec<String>,
    /// Tracker URL (empty if missing).
    pub tracker: String,
    /// Save path (empty if missing).
    pub path: String,
    /// Category (default `uncategorized` when missing).
    pub category: String,
    /// Size in bytes for the payload.
    pub size_bytes: u64,
    /// Upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Download throughput in bytes per second.
    pub download_bps: u64,
}

impl TorrentRow {
    /// Human-friendly size rounded to two decimal places.
    #[must_use]
    pub fn size_label(&self) -> String {
        const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
        let hundredths = self.size_bytes.saturating_mul(100) / BYTES_PER_GIB;
        let whole = hundredths / 100;
        let frac = hundredths % 100;
        format!("{whole}.{frac:02} GB")
    }
}

impl From<TorrentSummary> for TorrentRow {
    fn from(value: TorrentSummary) -> Self {
        Self {
            id: value.id,
            name: value.name,
            status: value.status,
            progress: value.progress,
            eta: value.eta_seconds.map(|eta| {
                if eta == 0 {
                    "–".to_string()
                } else {
                    format!("{eta}s")
                }
            }),
            ratio: value.ratio,
            tags: value.tags,
            tracker: value.tracker.unwrap_or_default(),
            path: value.save_path.unwrap_or_default(),
            category: value
                .category
                .unwrap_or_else(|| "uncategorized".to_string()),
            size_bytes: value.size_bytes,
            upload_bps: value.upload_bps,
            download_bps: value.download_bps,
        }
    }
}

/// Selection set used for bulk torrent actions.
pub type SelectionSet = HashSet<Uuid, RandomState>;

/// Current torrents slice stored in the app state.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TorrentsState {
    /// Map of torrent rows by id.
    pub by_id: HashMap<Uuid, Rc<TorrentRow>>,
    /// Ordered list of visible torrent ids.
    pub visible_ids: Vec<Uuid>,
    /// Multi-select set for bulk actions.
    pub selected: SelectionSet,
    /// Cached torrent detail data for the drawer.
    pub details_by_id: HashMap<Uuid, Rc<DetailData>>,
    /// Active filter state used for fetching and SSE filtering.
    pub filters: TorrentFilters,
    /// Current drawer selection id.
    pub selected_id: Option<Uuid>,
}

/// Filter state for the torrents list.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TorrentFilters {
    /// Search query string.
    pub search: String,
    /// Regex search toggle.
    pub regex: bool,
    /// Optional state filter.
    pub state: Option<String>,
}

/// Minimal progress update for coalesced SSE events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressPatch {
    /// Torrent id to update.
    pub id: Uuid,
    /// Completion ratio (0.0-1.0).
    pub progress: f32,
    /// Optional ETA in seconds.
    pub eta_seconds: Option<u64>,
    /// Optional download rate in bytes/sec.
    pub download_bps: Option<u64>,
    /// Optional upload rate in bytes/sec.
    pub upload_bps: Option<u64>,
}

/// Replace list rows with a new snapshot.
pub fn set_rows(state: &mut TorrentsState, rows: Vec<TorrentRow>) {
    state.visible_ids = rows.iter().map(|row| row.id).collect();
    state.by_id = rows.into_iter().map(|row| (row.id, Rc::new(row))).collect();
    state.selected.retain(|id| state.by_id.contains_key(id));
    if let Some(id) = state.selected_id
        && !state.by_id.contains_key(&id)
    {
        state.selected_id = None;
    }
}

/// Set the selected drawer id.
pub const fn set_selected_id(state: &mut TorrentsState, id: Option<Uuid>) {
    state.selected_id = id;
}

/// Upsert torrent detail payload.
pub fn upsert_detail(state: &mut TorrentsState, id: Uuid, detail: DetailData) {
    state.details_by_id.insert(id, Rc::new(detail));
}

/// Apply a coalesced progress patch to the list state.
pub fn apply_progress_patch(state: &mut TorrentsState, patch: ProgressPatch) {
    let Some(current) = state.by_id.get(&patch.id) else {
        return;
    };
    let mut next = (**current).clone();
    next.progress = patch.progress;
    next.eta = patch.eta_seconds.map(|eta| {
        if eta == 0 {
            "–".to_string()
        } else {
            format!("{eta}s")
        }
    });
    if let Some(download_bps) = patch.download_bps {
        next.download_bps = download_bps;
    }
    if let Some(upload_bps) = patch.upload_bps {
        next.upload_bps = upload_bps;
    }
    state.by_id.insert(patch.id, Rc::new(next));
}

/// Update the stored status for a torrent row.
pub fn update_status(state: &mut TorrentsState, id: Uuid, status: String) {
    let Some(current) = state.by_id.get(&id) else {
        return;
    };
    let mut next = (**current).clone();
    next.status = status;
    state.by_id.insert(id, Rc::new(next));
}

/// Update the stored metadata for a torrent row.
pub fn update_metadata(
    state: &mut TorrentsState,
    id: Uuid,
    name: Option<String>,
    download_dir: Option<String>,
) {
    let Some(current) = state.by_id.get(&id) else {
        return;
    };
    let mut next = (**current).clone();
    if let Some(name) = name {
        next.name = name;
    }
    if let Some(download_dir) = download_dir {
        next.path = download_dir;
    }
    state.by_id.insert(id, Rc::new(next));
}

/// Remove a torrent row from the list state.
pub fn remove_row(state: &mut TorrentsState, id: Uuid) {
    state.by_id.remove(&id);
    state.visible_ids.retain(|row_id| *row_id != id);
    state.details_by_id.remove(&id);
    state.selected.remove(&id);
    if state.selected_id == Some(id) {
        state.selected_id = None;
    }
}

/// Replace the current bulk-selection set.
pub fn set_selected(state: &mut TorrentsState, selected: SelectionSet) {
    state.selected = selected;
}

/// Read the visible torrent ids in list order.
#[must_use]
pub fn select_visible_ids(state: &TorrentsState) -> Vec<Uuid> {
    state.visible_ids.clone()
}

/// Read the visible torrent rows in list order.
#[must_use]
pub fn select_visible_rows(state: &TorrentsState) -> Vec<TorrentRow> {
    state
        .visible_ids
        .iter()
        .filter_map(|id| state.by_id.get(id).map(|row| (**row).clone()))
        .collect()
}

/// Read a row by id.
#[must_use]
pub fn select_torrent_row(state: &TorrentsState, id: &Uuid) -> Option<Rc<TorrentRow>> {
    state.by_id.get(id).cloned()
}

/// Read the selected detail payload for the drawer.
#[must_use]
pub fn select_selected_detail(state: &TorrentsState) -> Option<DetailData> {
    let id = state.selected_id?;
    state
        .details_by_id
        .get(&id)
        .map(|detail| (**detail).clone())
}

/// Apply progress/rate updates from SSE.
#[must_use]
pub fn apply_progress(
    rows: &[TorrentRow],
    id: Uuid,
    progress: f32,
    eta_seconds: Option<u64>,
    download_bps: Option<u64>,
    upload_bps: Option<u64>,
) -> Vec<TorrentRow> {
    rows.iter()
        .cloned()
        .map(|mut row| {
            if row.id == id {
                row.progress = progress;
                row.eta = eta_seconds.map(|eta| {
                    if eta == 0 {
                        "–".to_string()
                    } else {
                        format!("{eta}s")
                    }
                });
                if let Some(download_bps) = download_bps {
                    row.download_bps = download_bps;
                }
                if let Some(upload_bps) = upload_bps {
                    row.upload_bps = upload_bps;
                }
            }
            row
        })
        .collect()
}

/// Apply metadata updates such as name and download path.
#[must_use]
pub fn apply_metadata(
    rows: &[TorrentRow],
    id: Uuid,
    name: Option<&str>,
    download_dir: Option<&str>,
) -> Vec<TorrentRow> {
    rows.iter()
        .cloned()
        .map(|mut row| {
            if row.id == id {
                if let Some(name) = name {
                    row.name = name.to_string();
                }
                if let Some(path) = download_dir {
                    row.path = path.to_string();
                }
            }
            row
        })
        .collect()
}

/// Apply torrent rate updates from SSE.
#[must_use]
pub fn apply_rates(
    rows: &[TorrentRow],
    id: Uuid,
    download_bps: u64,
    upload_bps: u64,
) -> Vec<TorrentRow> {
    rows.iter()
        .cloned()
        .map(|mut row| {
            if row.id == id {
                row.download_bps = download_bps;
                row.upload_bps = upload_bps;
            }
            row
        })
        .collect()
}

/// Apply status/state updates from SSE.
#[must_use]
pub fn apply_status(rows: &[TorrentRow], id: Uuid, status: &str) -> Vec<TorrentRow> {
    rows.iter()
        .cloned()
        .map(|mut row| {
            if row.id == id {
                row.status = status.to_string();
            }
            row
        })
        .collect()
}

/// Remove torrent rows when SSE signals removal.
#[must_use]
pub fn apply_remove(rows: &[TorrentRow], id: Uuid) -> Vec<TorrentRow> {
    rows.iter().filter(|row| row.id != id).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::torrents::actions::{TorrentAction, success_message};
    use crate::i18n::{LocaleCode, TranslationBundle};
    use uuid::Uuid;

    const EPSILON: f32 = 0.000_1;
    const GIB: u64 = 1_073_741_824;

    fn base_row(id: Uuid) -> TorrentRow {
        TorrentRow {
            id,
            name: "alpha".into(),
            status: "downloading".into(),
            progress: 0.1,
            eta: Some("10s".into()),
            ratio: 0.0,
            tags: vec![],
            tracker: "t1".into(),
            path: "/data/a".into(),
            category: "tv".into(),
            size_bytes: GIB,
            upload_bps: 0,
            download_bps: 0,
        }
    }

    #[test]
    fn progress_updates_fields() {
        let id = Uuid::from_u128(1);
        let updated = apply_progress(&[base_row(id)], id, 0.25, Some(5), Some(10), Some(20));
        let first = updated.first().unwrap();
        assert!((first.progress - 0.25).abs() < EPSILON);
        assert_eq!(first.eta.as_deref(), Some("5s"));
        assert_eq!(first.download_bps, 10);
        assert_eq!(first.upload_bps, 20);
    }

    #[test]
    fn rates_update_only_target() {
        let id_one = Uuid::from_u128(1);
        let id_two = Uuid::from_u128(2);
        let updated = apply_rates(&[base_row(id_one), base_row(id_two)], id_two, 5, 9);
        let second = updated.iter().find(|r| r.id == id_two).unwrap();
        assert_eq!(second.download_bps, 5);
        assert_eq!(second.upload_bps, 9);
        let first = updated.iter().find(|r| r.id == id_one).unwrap();
        assert_eq!(first.download_bps, 0);
    }

    #[test]
    fn status_and_remove_work() {
        let id_one = Uuid::from_u128(1);
        let id_two = Uuid::from_u128(2);
        let status = apply_status(&[base_row(id_one), base_row(id_two)], id_two, "checking");
        assert_eq!(
            status.iter().find(|r| r.id == id_two).unwrap().status,
            "checking"
        );
        let removed = apply_remove(&status, id_one);
        assert_eq!(removed.len(), 1);
        assert!(removed.iter().all(|r| r.id == id_two));
    }

    #[test]
    fn metadata_updates_name_and_path() {
        let id = Uuid::from_u128(1);
        let updated = apply_metadata(&[base_row(id)], id, Some("beta"), Some("/new"));
        let first = updated.first().unwrap();
        assert_eq!(first.name, "beta");
        assert_eq!(first.path, "/new");
    }

    #[test]
    fn success_messages_are_localised() {
        let bundle = TranslationBundle::new(LocaleCode::En);
        assert!(
            success_message(&bundle, &TorrentAction::Pause, "alpha")
                .contains(&bundle.text("toast.pause", ""))
        );
        assert!(
            success_message(&bundle, &TorrentAction::Delete { with_data: true }, "alpha")
                .contains(&bundle.text("toast.delete_data", ""))
        );
    }

    #[test]
    fn summary_conversion_maps_sizes() {
        let summary = TorrentSummary {
            id: Uuid::nil(),
            name: "demo".into(),
            status: "paused".into(),
            progress: 1.0,
            eta_seconds: None,
            ratio: 1.1,
            tags: vec!["tag".into()],
            tracker: Some("t".into()),
            save_path: Some("/p".into()),
            category: Some("movies".into()),
            size_bytes: GIB,
            download_bps: 3,
            upload_bps: 4,
            added_at: None,
            completed_at: None,
        };
        let row: TorrentRow = summary.into();
        assert_eq!(row.size_bytes, GIB);
        assert_eq!(row.size_label(), "1.00 GB");
        assert_eq!(row.tracker, "t");
        assert_eq!(row.path, "/p");
    }
}
