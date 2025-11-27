//! Shared torrent models and pure state transformations for testing outside wasm.

use crate::i18n::TranslationBundle;
use crate::models::TorrentSummary;

/// UI-friendly torrent snapshot used across list/state helpers.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRow {
    /// Stable torrent identifier.
    pub id: String,
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

/// Torrent actions emitted from UI controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TorrentAction {
    /// Pause the torrent.
    Pause,
    /// Resume the torrent.
    Resume,
    /// Force a recheck.
    Recheck,
    /// Delete the torrent, optionally removing data.
    Delete {
        /// Whether payload data should also be removed.
        with_data: bool,
    },
}

impl From<TorrentSummary> for TorrentRow {
    fn from(value: TorrentSummary) -> Self {
        Self {
            id: value.id.to_string(),
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

/// Apply progress/rate updates from SSE.
#[must_use]
pub fn apply_progress(
    rows: &[TorrentRow],
    id: &str,
    progress: f32,
    eta_seconds: Option<u64>,
    download_bps: u64,
    upload_bps: u64,
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
                row.download_bps = download_bps;
                row.upload_bps = upload_bps;
            }
            row
        })
        .collect()
}

/// Apply torrent rate updates from SSE.
#[must_use]
pub fn apply_rates(
    rows: &[TorrentRow],
    id: &str,
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
pub fn apply_status(rows: &[TorrentRow], id: &str, status: &str) -> Vec<TorrentRow> {
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
pub fn apply_remove(rows: &[TorrentRow], id: &str) -> Vec<TorrentRow> {
    rows.iter().filter(|row| row.id != id).cloned().collect()
}

/// Format a toast message for a successful action.
#[must_use]
pub fn success_message(bundle: &TranslationBundle, action: &TorrentAction, name: &str) -> String {
    match action {
        TorrentAction::Pause => format!("{} {name}", bundle.text("toast.pause", "")),
        TorrentAction::Resume => format!("{} {name}", bundle.text("toast.resume", "")),
        TorrentAction::Recheck => format!("{} {name}", bundle.text("toast.recheck", "")),
        TorrentAction::Delete { with_data } => {
            if *with_data {
                format!("{} {name}", bundle.text("toast.delete_data", ""))
            } else {
                format!("{} {name}", bundle.text("toast.delete", ""))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{LocaleCode, TranslationBundle};
    use uuid::Uuid;

    const EPSILON: f32 = 0.000_1;
    const GIB: u64 = 1_073_741_824;

    fn base_row(id: &str) -> TorrentRow {
        TorrentRow {
            id: id.to_string(),
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
        let updated = apply_progress(&[base_row("1")], "1", 0.25, Some(5), 10, 20);
        let first = updated.first().unwrap();
        assert!((first.progress - 0.25).abs() < EPSILON);
        assert_eq!(first.eta.as_deref(), Some("5s"));
        assert_eq!(first.download_bps, 10);
        assert_eq!(first.upload_bps, 20);
    }

    #[test]
    fn rates_update_only_target() {
        let updated = apply_rates(&[base_row("1"), base_row("2")], "2", 5, 9);
        let second = updated.iter().find(|r| r.id == "2").unwrap();
        assert_eq!(second.download_bps, 5);
        assert_eq!(second.upload_bps, 9);
        let first = updated.iter().find(|r| r.id == "1").unwrap();
        assert_eq!(first.download_bps, 0);
    }

    #[test]
    fn status_and_remove_work() {
        let status = apply_status(&[base_row("1"), base_row("2")], "2", "checking");
        assert_eq!(
            status.iter().find(|r| r.id == "2").unwrap().status,
            "checking"
        );
        let removed = apply_remove(&status, "1");
        assert_eq!(removed.len(), 1);
        assert!(removed.iter().all(|r| r.id == "2"));
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
