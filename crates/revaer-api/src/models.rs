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
    /// URI reference identifying the problem type.
    pub kind: String,
    /// Short, human-readable summary of the issue.
    pub title: String,
    /// HTTP status code associated with the error.
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Detailed diagnostic message when available.
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Parameters that failed validation, if applicable.
    pub invalid_params: Option<Vec<ProblemInvalidParam>>,
}

/// Invalid parameter pointer surfaced alongside a [`ProblemDetails`] payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProblemInvalidParam {
    /// JSON Pointer to the offending field.
    pub pointer: String,
    /// Human-readable description of the validation failure.
    pub message: String,
}

/// Enumerates the coarse torrent lifecycle states surfaced via the API.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TorrentStateKind {
    /// Awaiting initial processing by the engine.
    Queued,
    /// Downloading metadata (e.g., contacting trackers / DHT).
    FetchingMetadata,
    /// Actively fetching pieces from the swarm.
    Downloading,
    /// Seeding to peers.
    Seeding,
    /// Completed and ready for post-processing.
    Completed,
    /// Encountered an unrecoverable failure.
    Failed,
    /// Paused or otherwise stopped without error.
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
    /// Normalised lifecycle state label.
    pub kind: TorrentStateKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional failure context if the torrent stopped unexpectedly.
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
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes expected for the torrent.
    pub bytes_total: u64,
    /// Percentage (0.0–100.0) of completion.
    pub percent_complete: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Estimated time to completion in seconds, when calculable.
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
    /// Current download throughput in bytes per second.
    pub download_bps: u64,
    /// Current upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Share ratio calculated as uploaded/downloaded.
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
    /// Zero-based index assigned by the engine.
    pub index: u32,
    /// Normalised relative path of the file inside the torrent.
    pub path: String,
    /// Total size of the file in bytes.
    pub size_bytes: u64,
    /// Number of bytes downloaded so far.
    pub bytes_completed: u64,
    /// Requested priority level for the file.
    pub priority: FilePriority,
    /// Indicates whether the file is currently selected for download.
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
    /// Stable identifier for the torrent.
    pub id: Uuid,
    /// Human-friendly name if present.
    pub name: Option<String>,
    /// Current lifecycle state of the torrent.
    pub state: TorrentStateView,
    /// Transfer progress statistics.
    pub progress: TorrentProgressView,
    /// Observed bandwidth figures.
    pub rates: TorrentRatesView,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Absolute path to the library artifact once finalised.
    pub library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Active download root path.
    pub download_dir: Option<String>,
    /// Whether sequential mode is enabled.
    pub sequential: bool,
    #[serde(default)]
    /// Tags associated with the torrent.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Tracker URLs recorded for the torrent.
    pub trackers: Vec<String>,
    /// Timestamp when the torrent was registered with the engine.
    pub added_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Time the torrent completed, if known.
    pub completed_at: Option<DateTime<Utc>>,
    /// Timestamp of the latest status update.
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
    /// Summary information for the torrent.
    pub summary: TorrentSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Detailed file breakdown if requested.
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
    /// Page of torrent summaries.
    pub torrents: Vec<TorrentSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Cursor for retrieving the next page, when available.
    pub next: Option<String>,
}

/// JSON body accepted by `POST /v1/torrents` when carrying a magnet URI or
/// base64-encoded `.torrent` metainfo payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentCreateRequest {
    /// Client-provided identifier for idempotent operations.
    pub id: Uuid,
    #[serde(default)]
    /// Magnet URI used to describe the torrent.
    pub magnet: Option<String>,
    #[serde(default)]
    /// Base64-encoded `.torrent` payload.
    pub metainfo: Option<String>,
    #[serde(default)]
    /// Friendly display name override.
    pub name: Option<String>,
    #[serde(default)]
    /// Optional download directory to stage content.
    pub download_dir: Option<String>,
    #[serde(default)]
    /// Enables sequential download mode on creation when set.
    pub sequential: Option<bool>,
    #[serde(default)]
    /// Tags to associate with the torrent immediately.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Additional tracker URLs to register.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be selected during the initial download.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that must be excluded from the download set.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Indicates whether the built-in fluff filtering preset should be applied.
    pub skip_fluff: bool,
    #[serde(default)]
    /// Optional download bandwidth cap in bytes per second.
    pub max_download_bps: Option<u64>,
    #[serde(default)]
    /// Optional upload bandwidth cap in bytes per second.
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
    /// Glob patterns that must remain selected.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be deselected.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Overrides the skip-fluff preset when present.
    pub skip_fluff: Option<bool>,
    #[serde(default)]
    /// Explicit per-file priority overrides.
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
    /// Pause the torrent without removing any data.
    Pause,
    /// Resume a previously paused torrent.
    Resume,
    /// Remove the torrent and optionally delete its data.
    Remove {
        #[serde(default)]
        /// Flag indicating whether to delete downloaded files as well.
        delete_data: bool,
    },
    /// Force a reannounce to trackers.
    Reannounce,
    /// Schedule a full recheck of the torrent contents.
    Recheck,
    /// Toggle sequential download mode.
    Sequential {
        /// Enables sequential reading when `true`.
        enable: bool,
    },
    /// Adjust torrent or global bandwidth limits.
    Rate {
        #[serde(default)]
        /// Download cap in bytes per second.
        download_bps: Option<u64>,
        #[serde(default)]
        /// Upload cap in bytes per second.
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
