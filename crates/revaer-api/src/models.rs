//! Shared HTTP DTOs for the Revaer public API.
//!
//! These types are re-used by the CLI for request/response encoding to keep the
//! contract deterministic. The conversions live close to the server so the
//! mapping from domain objects (`TorrentStatus`, `FileSelectionUpdate`, etc.)
//! remains a single source of truth.
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use revaer_events::TorrentState;
use revaer_torrent_core::{
    AddTorrentOptions, FilePriority, FilePriorityOverride, FileSelectionRules, FileSelectionUpdate,
    TorrentRateLimit, TorrentSource, TorrentStatus,
};

/// RFC9457-compatible problem document surfaced on validation/runtime errors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_params: Option<Vec<ProblemInvalidParam>>,
}

/// Invalid parameter pointer surfaced alongside a [`ProblemDetails`] payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemInvalidParam {
    pub pointer: String,
    pub message: String,
}

/// Enumerates the coarse torrent lifecycle states surfaced via the API.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TorrentStateKind {
    Queued,
    FetchingMetadata,
    Downloading,
    Seeding,
    Completed,
    Failed,
    Stopped,
}

impl From<TorrentState> for TorrentStateKind {
    fn from(value: TorrentState) -> Self {
        match value {
            TorrentState::Queued => Self::Queued,
            TorrentState::FetchingMetadata => Self::FetchingMetadata,
            TorrentState::Downloading => Self::Downloading,
            TorrentState::Seeding => Self::Seeding,
            TorrentState::Completed => Self::Completed,
            TorrentState::Failed { .. } => Self::Failed,
            TorrentState::Stopped => Self::Stopped,
        }
    }
}

/// Describes the state + optional failure message for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentStateView {
    pub kind: TorrentStateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
}

impl From<TorrentState> for TorrentStateView {
    fn from(value: TorrentState) -> Self {
        let kind = TorrentStateKind::from(value.clone());
        let failure_message = match value {
            TorrentState::Failed { message } => Some(message),
            _ => None,
        };
        Self {
            kind,
            failure_message,
        }
    }
}

/// Aggregated progress metrics for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentProgressView {
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub percent_complete: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
}

impl From<&TorrentStatus> for TorrentProgressView {
    fn from(status: &TorrentStatus) -> Self {
        Self {
            bytes_downloaded: status.progress.bytes_downloaded,
            bytes_total: status.progress.bytes_total,
            percent_complete: status.progress.percent_complete(),
            eta_seconds: status.progress.eta_seconds,
        }
    }
}

/// Transfer rates surfaced with a torrent snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentRatesView {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub ratio: f64,
}

impl From<&TorrentStatus> for TorrentRatesView {
    fn from(status: &TorrentStatus) -> Self {
        Self {
            download_bps: status.rates.download_bps,
            upload_bps: status.rates.upload_bps,
            ratio: status.rates.ratio,
        }
    }
}

/// File metadata returned when the client requests detailed torrent views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentFileView {
    pub index: u32,
    pub path: String,
    pub size_bytes: u64,
    pub bytes_completed: u64,
    pub priority: FilePriority,
    pub selected: bool,
}

impl From<revaer_torrent_core::TorrentFile> for TorrentFileView {
    fn from(file: revaer_torrent_core::TorrentFile) -> Self {
        Self {
            index: file.index,
            path: file.path,
            size_bytes: file.size_bytes,
            bytes_completed: file.bytes_completed,
            priority: file.priority,
            selected: file.selected,
        }
    }
}

/// High-level view returned when listing torrents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub state: TorrentStateView,
    pub progress: TorrentProgressView,
    pub rates: TorrentRatesView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
    pub sequential: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub trackers: Vec<String>,
    pub added_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
}

impl From<TorrentStatus> for TorrentSummary {
    fn from(status: TorrentStatus) -> Self {
        Self {
            id: status.id,
            name: status.name.clone(),
            state: TorrentStateView::from(status.state.clone()),
            progress: TorrentProgressView::from(&status),
            rates: TorrentRatesView::from(&status),
            library_path: status.library_path.clone(),
            download_dir: status.download_dir.clone(),
            sequential: status.sequential,
            tags: Vec::new(),
            trackers: Vec::new(),
            added_at: status.added_at,
            completed_at: status.completed_at,
            last_updated: status.last_updated,
        }
    }
}

impl TorrentSummary {
    /// Attach API-layer metadata (tags/trackers) captured alongside the torrent.
    #[must_use]
    pub fn with_metadata(mut self, tags: Vec<String>, trackers: Vec<String>) -> Self {
        self.tags = tags;
        self.trackers = trackers;
        self
    }
}

/// Full detail view returned when querying a specific torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentDetail {
    #[serde(flatten)]
    pub summary: TorrentSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<TorrentFileView>>,
}

impl From<TorrentStatus> for TorrentDetail {
    fn from(status: TorrentStatus) -> Self {
        let summary = TorrentSummary::from(status.clone());
        let files = status
            .files
            .map(|items| items.into_iter().map(TorrentFileView::from).collect());
        Self { summary, files }
    }
}

/// Paginated list response for the torrent collection endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentListResponse {
    pub torrents: Vec<TorrentSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// JSON body accepted by `POST /v1/torrents` when carrying a magnet URI or
/// base64-encoded `.torrent` metainfo payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentCreateRequest {
    pub id: Uuid,
    #[serde(default)]
    pub magnet: Option<String>,
    #[serde(default)]
    pub metainfo: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub download_dir: Option<String>,
    #[serde(default)]
    pub sequential: Option<bool>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub trackers: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub skip_fluff: bool,
    #[serde(default)]
    pub max_download_bps: Option<u64>,
    #[serde(default)]
    pub max_upload_bps: Option<u64>,
}

impl TorrentCreateRequest {
    /// Translate the client payload into the engine-facing [`AddTorrentOptions`].
    #[must_use]
    pub fn to_options(&self) -> AddTorrentOptions {
        AddTorrentOptions {
            name_hint: self.name.clone(),
            download_dir: self.download_dir.clone(),
            sequential: self.sequential,
            file_rules: FileSelectionRules {
                include: self.include.clone(),
                exclude: self.exclude.clone(),
                skip_fluff: self.skip_fluff,
            },
            rate_limit: TorrentRateLimit {
                download_bps: self.max_download_bps,
                upload_bps: self.max_upload_bps,
            },
            tags: self.tags.clone(),
        }
    }

    /// Establish the [`TorrentSource`] from the payload.
    ///
    /// Returns `None` if neither a magnet URI nor metainfo payload is provided.
    #[must_use]
    pub fn to_source(&self) -> Option<TorrentSource> {
        self.magnet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map_or_else(
                || {
                    self.metainfo.as_ref().and_then(|encoded| {
                        general_purpose::STANDARD
                            .decode(encoded)
                            .map(TorrentSource::metainfo)
                            .ok()
                    })
                },
                |magnet| Some(TorrentSource::magnet(magnet.to_string())),
            )
    }
}

/// Body accepted by `POST /v1/torrents/{id}/select`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentSelectionRequest {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub skip_fluff: Option<bool>,
    #[serde(default)]
    pub priorities: Vec<FilePriorityOverride>,
}

impl From<TorrentSelectionRequest> for FileSelectionUpdate {
    fn from(request: TorrentSelectionRequest) -> Self {
        Self {
            include: request.include,
            exclude: request.exclude,
            skip_fluff: request.skip_fluff.unwrap_or(false),
            priorities: request.priorities,
        }
    }
}

/// Envelope describing the action a client wants to perform on a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TorrentAction {
    Pause,
    Resume,
    Remove {
        #[serde(default)]
        delete_data: bool,
    },
    Reannounce,
    Recheck,
    Sequential {
        enable: bool,
    },
    Rate {
        #[serde(default)]
        download_bps: Option<u64>,
        #[serde(default)]
        upload_bps: Option<u64>,
    },
}

impl TorrentAction {
    /// Translate the action into a [`TorrentRateLimit`] when applicable.
    #[must_use]
    pub const fn to_rate_limit(&self) -> Option<TorrentRateLimit> {
        match self {
            Self::Rate {
                download_bps,
                upload_bps,
            } => Some(TorrentRateLimit {
                download_bps: *download_bps,
                upload_bps: *upload_bps,
            }),
            _ => None,
        }
    }
}
