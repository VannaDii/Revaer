//! Shared torrent models and pure state transformations for testing outside wasm.

use crate::models::{DetailData, FileNode, TorrentStateKind, TorrentStateView, TorrentSummary};
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
    pub progress: f64,
    /// Human-readable ETA or en dash when unknown.
    pub eta: Option<String>,
    /// Current ratio for the torrent.
    pub ratio: f64,
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
        let progress = clamp_f64(value.progress.percent_complete / 100.0);
        Self {
            id: value.id,
            name: value.name.unwrap_or_else(|| "<unspecified>".to_string()),
            status: format_state_view(&value.state),
            progress,
            eta: value.progress.eta_seconds.map(|eta| {
                if eta == 0 {
                    "–".to_string()
                } else {
                    format!("{eta}s")
                }
            }),
            ratio: clamp_f64(value.rates.ratio),
            tags: value.tags,
            tracker: value.trackers.first().cloned().unwrap_or_default(),
            path: value
                .download_dir
                .or(value.library_path)
                .unwrap_or_default(),
            category: value
                .category
                .unwrap_or_else(|| "uncategorized".to_string()),
            size_bytes: value.progress.bytes_total,
            upload_bps: value.rates.upload_bps,
            download_bps: value.rates.download_bps,
        }
    }
}

const fn clamp_f64(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.max(0.0)
}

fn format_state_view(state: &TorrentStateView) -> String {
    let label = match state.kind {
        TorrentStateKind::Queued => "queued",
        TorrentStateKind::FetchingMetadata => "fetching_metadata",
        TorrentStateKind::Downloading => "downloading",
        TorrentStateKind::Seeding => "seeding",
        TorrentStateKind::Completed => "completed",
        TorrentStateKind::Failed => "failed",
        TorrentStateKind::Stopped => "stopped",
    };
    if matches!(state.kind, TorrentStateKind::Failed)
        && let Some(message) = state.failure_message.as_ref()
    {
        return format!("failed: {message}");
    }
    label.to_string()
}

/// Static metadata slice for list rows.
#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRowBase {
    /// Stable torrent identifier.
    pub id: Uuid,
    /// Display name for the torrent payload.
    pub name: String,
    /// Tracker URL (empty if missing).
    pub tracker: String,
    /// Any labels applied to the torrent.
    pub tags: Vec<String>,
    /// Save path (empty if missing).
    pub path: String,
    /// Category (default `uncategorized` when missing).
    pub category: String,
    /// Total payload size in bytes.
    pub size_bytes: u64,
    /// Upload ratio as reported by the engine.
    pub ratio: f64,
}

impl TorrentRowBase {
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

/// Fast-changing slice for progress and throughput values.
#[derive(Clone, Debug, PartialEq)]
pub struct TorrentProgressSlice {
    /// Torrent status string.
    pub status: String,
    /// Completion percentage in the range 0.0–1.0.
    pub progress: f64,
    /// Human-readable ETA or en dash when unknown.
    pub eta: Option<String>,
    /// Upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Download throughput in bytes per second.
    pub download_bps: u64,
}

/// Selection set used for bulk torrent actions.
pub type SelectionSet = HashSet<Uuid, RandomState>;

/// Query model mirrored into the URL and request query params.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TorrentsQueryModel {
    /// Search by name (substring).
    pub name: String,
    /// Optional lifecycle state filter.
    pub state: Option<String>,
    /// Optional tag filters.
    pub tags: Vec<String>,
    /// Optional tracker filter.
    pub tracker: Option<String>,
    /// Optional file extension filter.
    pub extension: Option<String>,
}

/// Paging state for the torrent list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentsPaging {
    /// Cursor used for the next list request.
    pub cursor: Option<String>,
    /// Cursor for the next page, returned by the API.
    pub next_cursor: Option<String>,
    /// Page size limit for list requests.
    pub limit: u32,
    /// Busy flag for list requests.
    pub is_loading: bool,
}

impl Default for TorrentsPaging {
    fn default() -> Self {
        Self {
            cursor: None,
            next_cursor: None,
            limit: 50,
            is_loading: false,
        }
    }
}

/// Filesystem post-processing snapshot for a torrent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsopsState {
    /// Current filesystem processing status.
    pub status: FsopsStatus,
    /// Optional last step label.
    pub step: Option<String>,
    /// Optional error message.
    pub error: Option<String>,
}

/// Status variants for filesystem processing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsopsStatus {
    /// Filesystem processing is in progress.
    InProgress,
    /// Filesystem processing completed successfully.
    Completed,
    /// Filesystem processing failed.
    Failed,
}

/// Compact filesystem-processing badge for list rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsopsBadge {
    /// Current filesystem processing status.
    pub status: FsopsStatus,
    /// Optional detail to surface (step or error message).
    pub detail: Option<String>,
}

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
    /// Filesystem processing state per torrent.
    pub fsops_by_id: HashMap<Uuid, Rc<FsopsState>>,
    /// Active filter state used for fetching and SSE filtering.
    pub filters: TorrentsQueryModel,
    /// Pagination state for list requests.
    pub paging: TorrentsPaging,
    /// Current drawer selection id.
    pub selected_id: Option<Uuid>,
}

/// Minimal progress update for coalesced SSE events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressPatch {
    /// Torrent id to update.
    pub id: Uuid,
    /// Completion ratio (0.0-1.0).
    pub progress: f64,
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
    state
        .fsops_by_id
        .retain(|id, _| state.by_id.contains_key(id));
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

/// Update file selection state for a cached detail payload.
pub fn update_detail_file_selection(state: &mut TorrentsState, id: Uuid, path: &str, wanted: bool) {
    let Some(current) = state.details_by_id.get(&id) else {
        return;
    };
    let mut next = (**current).clone();
    if update_file_nodes(&mut next.files, path, wanted) {
        state.details_by_id.insert(id, Rc::new(next));
    }
}

fn update_file_nodes(nodes: &mut [FileNode], path: &str, wanted: bool) -> bool {
    let mut changed = false;
    for node in nodes {
        if node.name == path && node.wanted != wanted {
            node.wanted = wanted;
            changed = true;
        }
        if !node.children.is_empty() && update_file_nodes(&mut node.children, path, wanted) {
            changed = true;
        }
    }
    changed
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

/// Record filesystem processing start for a torrent.
pub fn update_fsops_started(state: &mut TorrentsState, id: Uuid) {
    state.fsops_by_id.insert(
        id,
        Rc::new(FsopsState {
            status: FsopsStatus::InProgress,
            step: None,
            error: None,
        }),
    );
}

/// Record filesystem processing progress for a torrent.
pub fn update_fsops_progress(state: &mut TorrentsState, id: Uuid, step: String) {
    state.fsops_by_id.insert(
        id,
        Rc::new(FsopsState {
            status: FsopsStatus::InProgress,
            step: Some(step),
            error: None,
        }),
    );
}

/// Record filesystem processing completion for a torrent.
pub fn update_fsops_completed(state: &mut TorrentsState, id: Uuid) {
    state.fsops_by_id.insert(
        id,
        Rc::new(FsopsState {
            status: FsopsStatus::Completed,
            step: None,
            error: None,
        }),
    );
}

/// Record filesystem processing failure for a torrent.
pub fn update_fsops_failed(state: &mut TorrentsState, id: Uuid, message: String) {
    state.fsops_by_id.insert(
        id,
        Rc::new(FsopsState {
            status: FsopsStatus::Failed,
            step: None,
            error: Some(message),
        }),
    );
}

/// Remove a torrent row from the list state.
pub fn remove_row(state: &mut TorrentsState, id: Uuid) {
    state.by_id.remove(&id);
    state.visible_ids.retain(|row_id| *row_id != id);
    state.details_by_id.remove(&id);
    state.fsops_by_id.remove(&id);
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

/// Read the static metadata slice for a torrent row.
#[must_use]
pub fn select_torrent_row_base(state: &TorrentsState, id: &Uuid) -> Option<TorrentRowBase> {
    let row = state.by_id.get(id)?;
    Some(TorrentRowBase {
        id: row.id,
        name: row.name.clone(),
        tracker: row.tracker.clone(),
        tags: row.tags.clone(),
        path: row.path.clone(),
        category: row.category.clone(),
        size_bytes: row.size_bytes,
        ratio: row.ratio,
    })
}

/// Read the progress slice for a torrent row.
#[must_use]
pub fn select_torrent_progress_slice(
    state: &TorrentsState,
    id: &Uuid,
) -> Option<TorrentProgressSlice> {
    let row = state.by_id.get(id)?;
    Some(TorrentProgressSlice {
        status: row.status.clone(),
        progress: row.progress,
        eta: row.eta.clone(),
        upload_bps: row.upload_bps,
        download_bps: row.download_bps,
    })
}

/// Read the filesystem-processing badge for a torrent row.
#[must_use]
pub fn select_fsops_badge(state: &TorrentsState, id: &Uuid) -> Option<FsopsBadge> {
    let fsops = state.fsops_by_id.get(id)?;
    let detail = match fsops.status {
        FsopsStatus::Failed => fsops.error.clone(),
        FsopsStatus::InProgress => fsops.step.clone(),
        FsopsStatus::Completed => None,
    };
    Some(FsopsBadge {
        status: fsops.status.clone(),
        detail,
    })
}

/// Check if a torrent id is selected for bulk actions.
#[must_use]
pub fn select_is_selected(state: &TorrentsState, id: &Uuid) -> bool {
    state.selected.contains(id)
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
    progress: f64,
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
    use crate::models::{TorrentProgressView, TorrentRatesView};
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    const EPSILON: f64 = 0.000_1;
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
        let now = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
        let summary = TorrentSummary {
            id: Uuid::nil(),
            name: Some("demo".into()),
            state: TorrentStateView {
                kind: TorrentStateKind::Stopped,
                failure_message: None,
            },
            progress: TorrentProgressView {
                bytes_downloaded: GIB,
                bytes_total: GIB,
                percent_complete: 100.0,
                eta_seconds: None,
            },
            rates: TorrentRatesView {
                download_bps: 3,
                upload_bps: 4,
                ratio: 1.1,
            },
            library_path: None,
            download_dir: Some("/p".into()),
            sequential: false,
            tags: vec!["tag".into()],
            category: Some("movies".into()),
            trackers: vec!["t".into()],
            rate_limit: None,
            connections_limit: None,
            added_at: now,
            completed_at: None,
            last_updated: now,
        };
        let row: TorrentRow = summary.into();
        assert_eq!(row.size_bytes, GIB);
        assert_eq!(row.size_label(), "1.00 GB");
        assert_eq!(row.tracker, "t");
        assert_eq!(row.path, "/p");
    }

    #[test]
    fn selectors_split_base_and_progress() {
        let id = Uuid::from_u128(42);
        let mut state = TorrentsState::default();
        set_rows(&mut state, vec![base_row(id)]);
        state.selected.insert(id);

        let base = select_torrent_row_base(&state, &id).expect("base slice");
        assert_eq!(base.name, "alpha");
        assert_eq!(base.tracker, "t1");

        let progress = select_torrent_progress_slice(&state, &id).expect("progress slice");
        assert_eq!(progress.status, "downloading");
        assert_eq!(progress.download_bps, 0);

        assert!(select_is_selected(&state, &id));
    }
}
