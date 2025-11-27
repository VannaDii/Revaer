//! Shared torrent row representation used by UI and API adapters.
#![allow(clippy::module_name_repetitions)]

use crate::models::TorrentSummary;

/// UI-friendly torrent snapshot used across list/state helpers.
#[derive(Clone, Debug, PartialEq)]
pub struct TorrentRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub progress: f32,
    pub eta: Option<String>,
    pub ratio: f32,
    pub tags: Vec<String>,
    pub tracker: String,
    pub path: String,
    pub category: String,
    pub size_gb: f32,
    pub upload_bps: u64,
    pub download_bps: u64,
}

/// Torrent actions emitted from UI controls.
#[derive(Clone, Debug, PartialEq)]
pub enum TorrentAction {
    Pause,
    Resume,
    Recheck,
    Delete { with_data: bool },
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
            size_gb: value.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
            upload_bps: value.upload_bps,
            download_bps: value.download_bps,
        }
    }
}
