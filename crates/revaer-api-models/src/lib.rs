#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
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
    AddTorrentOptions, FileSelectionRules, FileSelectionUpdate, PeerChoke, PeerInterest,
    PeerSnapshot, StorageMode, TorrentSource, TorrentStatus,
    model::{
        TorrentAuthorRequest as CoreTorrentAuthorRequest,
        TorrentAuthorResult as CoreTorrentAuthorResult, TorrentOptionsUpdate,
        TorrentTrackersUpdate, TorrentWebSeedsUpdate,
    },
};
pub use revaer_torrent_core::{
    FilePriority, FilePriorityOverride, TorrentCleanupPolicy, TorrentLabelPolicy, TorrentRateLimit,
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

/// Current selection rules applied to a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentSelectionView {
    #[serde(default)]
    /// Glob-style patterns that force inclusion.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob-style patterns that force exclusion.
    pub exclude: Vec<String>,
    /// Indicates whether fluff filtering is enabled.
    pub skip_fluff: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Explicit per-file priority overrides.
    pub priorities: Vec<FilePriorityOverride>,
}

impl From<&FileSelectionUpdate> for TorrentSelectionView {
    fn from(selection: &FileSelectionUpdate) -> Self {
        Self {
            include: selection.include.clone(),
            exclude: selection.exclude.clone(),
            skip_fluff: selection.skip_fluff,
            priorities: selection.priorities.clone(),
        }
    }
}

/// Snapshot of the configurable settings applied to a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TorrentSettingsView {
    #[serde(default)]
    /// Tags associated with the torrent.
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Trackers recorded for the torrent.
    pub trackers: Vec<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    /// Tracker messages/errors keyed by URL.
    pub tracker_messages: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent bandwidth limits when present.
    pub rate_limit: Option<TorrentRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent peer connection cap when configured.
    pub connections_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Download directory applied at admission time.
    pub download_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Comment captured from the torrent metainfo.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Source label captured from the torrent metainfo.
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Private flag captured from the torrent metainfo.
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Storage allocation mode applied to the torrent.
    pub storage_mode: Option<StorageMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether partfiles are enabled for this torrent.
    pub use_partfile: Option<bool>,
    /// Whether sequential mode is currently active.
    pub sequential: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// File selection rules most recently requested.
    pub selection: Option<TorrentSelectionView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether super-seeding is enabled for the torrent.
    pub super_seeding: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the torrent was admitted in seed mode.
    pub seed_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional seeding time stop threshold in seconds.
    pub seed_time_limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional cleanup policy applied after seeding thresholds are met.
    pub cleanup: Option<TorrentCleanupPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the torrent is auto-managed by the queue.
    pub auto_managed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether peer exchange is enabled for the torrent.
    pub pex_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    /// Web seeds attached to the torrent.
    pub web_seeds: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Tracker URLs recorded for the torrent.
    pub trackers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent rate cap overrides applied on admission.
    pub rate_limit: Option<revaer_torrent_core::TorrentRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent peer connection cap applied on admission.
    pub connections_limit: Option<i32>,
    /// Timestamp when the torrent was registered with the engine.
    pub added_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Time the torrent completed, if known.
    pub completed_at: Option<DateTime<Utc>>,
    /// Timestamp of the latest status update.
    pub last_updated: DateTime<Utc>,
}

/// Policy entry describing a category or tag label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentLabelEntry {
    /// Label name.
    pub name: String,
    #[serde(default)]
    /// Policy defaults applied when the label is used.
    pub policy: TorrentLabelPolicy,
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
            category: None,
            trackers: Vec::new(),
            rate_limit: None,
            connections_limit: None,
            added_at: status.added_at,
            completed_at: status.completed_at,
            last_updated: status.last_updated,
        }
    }
}

impl TorrentSummary {
    /// Attach API-layer metadata (tags/trackers) captured alongside the torrent.
    #[must_use]
    pub fn with_metadata(
        mut self,
        tags: Vec<String>,
        category: Option<String>,
        trackers: Vec<String>,
        rate_limit: Option<revaer_torrent_core::TorrentRateLimit>,
        connections_limit: Option<i32>,
    ) -> Self {
        self.tags = tags;
        self.category = category;
        self.trackers = trackers;
        self.rate_limit = rate_limit;
        self.connections_limit = connections_limit;
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
    /// Current configurable settings applied to the torrent.
    pub settings: Option<TorrentSettingsView>,
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
        let settings = TorrentSettingsView {
            tags: Vec::new(),
            category: None,
            trackers: Vec::new(),
            tracker_messages: std::collections::HashMap::new(),
            rate_limit: None,
            connections_limit: None,
            download_dir: status.download_dir.clone(),
            comment: status.comment.clone(),
            source: status.source.clone(),
            private: status.private,
            storage_mode: None,
            use_partfile: None,
            sequential: status.sequential,
            selection: None,
            super_seeding: None,
            seed_mode: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            cleanup: None,
            auto_managed: None,
            queue_position: None,
            pex_enabled: None,
            web_seeds: Vec::new(),
        };
        Self {
            summary,
            settings: Some(settings),
            files,
        }
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
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
    /// Optional comment override for torrent metadata.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source override for torrent metadata.
    pub source: Option<String>,
    #[serde(default)]
    /// Optional private flag override for torrent metadata.
    pub private: Option<bool>,
    #[serde(default)]
    /// Optional download directory to stage content.
    pub download_dir: Option<String>,
    #[serde(default)]
    /// Optional storage allocation mode override.
    pub storage_mode: Option<StorageMode>,
    #[serde(default)]
    /// Enables sequential download mode on creation when set.
    pub sequential: Option<bool>,
    #[serde(default)]
    /// Adds the torrent in a paused/queued state when true.
    pub start_paused: Option<bool>,
    #[serde(default)]
    /// Adds the torrent in seed mode (assumes data is complete).
    pub seed_mode: Option<bool>,
    #[serde(default)]
    /// Percentage of pieces to hash-check before honoring seed mode.
    pub hash_check_sample_pct: Option<u8>,
    #[serde(default)]
    /// Enables super-seeding on admission when set.
    pub super_seeding: Option<bool>,
    #[serde(default)]
    /// Tags to associate with the torrent immediately.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Optional category assigned to the torrent.
    pub category: Option<String>,
    #[serde(default)]
    /// Additional tracker URLs to register.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Whether the supplied trackers should replace profile defaults.
    pub replace_trackers: bool,
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
    #[serde(default)]
    /// Optional per-torrent peer connection limit.
    pub max_connections: Option<i32>,
    #[serde(default)]
    /// Optional share ratio threshold before stopping seeding.
    pub seed_ratio_limit: Option<f64>,
    #[serde(default)]
    /// Optional seeding time limit in seconds.
    pub seed_time_limit: Option<u64>,
    #[serde(default)]
    /// Optional override for auto-managed queueing.
    pub auto_managed: Option<bool>,
    #[serde(default)]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(default)]
    /// Optional override for peer exchange behaviour.
    pub pex_enabled: Option<bool>,
    #[serde(default)]
    /// Optional list of web seeds to attach on admission.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Whether supplied web seeds should replace existing seeds.
    pub replace_web_seeds: bool,
}

impl TorrentCreateRequest {
    /// Translate the client payload into the engine-facing [`AddTorrentOptions`].
    #[must_use]
    pub fn to_options(&self) -> AddTorrentOptions {
        let tags = self
            .tags
            .iter()
            .map(|tag| tag.trim())
            .filter(|tag| !tag.is_empty())
            .map(ToString::to_string)
            .collect();
        let category = self
            .category
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        AddTorrentOptions {
            name_hint: self.name.clone(),
            comment: self.comment.clone(),
            source: self.source.clone(),
            private: self.private,
            category,
            download_dir: self.download_dir.clone(),
            storage_mode: self.storage_mode,
            sequential: self.sequential,
            start_paused: self.start_paused,
            seed_mode: self.seed_mode,
            hash_check_sample_pct: self
                .hash_check_sample_pct
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            super_seeding: self.super_seeding,
            file_rules: FileSelectionRules {
                include: self.include.clone(),
                exclude: self.exclude.clone(),
                skip_fluff: self.skip_fluff,
            },
            rate_limit: TorrentRateLimit {
                download_bps: self.max_download_bps,
                upload_bps: self.max_upload_bps,
            },
            connections_limit: self
                .max_connections
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            seed_ratio_limit: self.seed_ratio_limit,
            seed_time_limit: self.seed_time_limit,
            auto_managed: self.auto_managed,
            queue_position: self.queue_position,
            pex_enabled: self.pex_enabled,
            web_seeds: self.web_seeds.clone(),
            replace_web_seeds: self.replace_web_seeds,
            tracker_auth: None,
            tags,
            cleanup: None,
            trackers: Vec::new(),
            replace_trackers: self.replace_trackers,
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

/// JSON body accepted by `POST /v1/torrents/create` to author a new torrent file.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentAuthorRequest {
    /// Local filesystem path to a file or directory to hash.
    pub root_path: String,
    #[serde(default)]
    /// Tracker URLs to embed in the metainfo.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Web seed URLs to embed in the metainfo.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be included.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob patterns that should be excluded.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Whether the skip-fluff preset should be applied.
    pub skip_fluff: bool,
    #[serde(default)]
    /// Optional piece length override in bytes.
    pub piece_length: Option<u32>,
    #[serde(default)]
    /// Whether to mark the torrent as private.
    pub private: bool,
    #[serde(default)]
    /// Optional comment embedded in the metainfo.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source label embedded in the metainfo.
    pub source: Option<String>,
}

impl TorrentAuthorRequest {
    /// Translate the request payload into a core authoring request.
    #[must_use]
    pub fn to_core(&self) -> CoreTorrentAuthorRequest {
        CoreTorrentAuthorRequest {
            root_path: self.root_path.clone(),
            trackers: self.trackers.clone(),
            web_seeds: self.web_seeds.clone(),
            file_rules: FileSelectionRules {
                include: self.include.clone(),
                exclude: self.exclude.clone(),
                skip_fluff: self.skip_fluff,
            },
            piece_length: self.piece_length,
            private: self.private,
            comment: self.comment.clone(),
            source: self.source.clone(),
        }
    }
}

/// File entry returned in a torrent authoring response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentAuthorFileView {
    /// Relative file path inside the torrent.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Response returned by `POST /v1/torrents/create`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentAuthorResponse {
    /// Base64-encoded metainfo payload.
    pub metainfo: String,
    /// Magnet URI derived from the metainfo.
    pub magnet_uri: String,
    /// Best available info hash string.
    pub info_hash: String,
    /// Effective piece length in bytes.
    pub piece_length: u32,
    /// Total payload size in bytes.
    pub total_size: u64,
    #[serde(default)]
    /// Files included in the torrent.
    pub files: Vec<TorrentAuthorFileView>,
    #[serde(default)]
    /// Warnings generated during authoring.
    pub warnings: Vec<String>,
    #[serde(default)]
    /// Trackers embedded in the metainfo.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Web seeds embedded in the metainfo.
    pub web_seeds: Vec<String>,
    /// Private flag embedded in the metainfo.
    pub private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Comment embedded in the metainfo.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Source label embedded in the metainfo.
    pub source: Option<String>,
}

impl TorrentAuthorResponse {
    #[must_use]
    /// Convert the core authoring result into an API response payload.
    pub fn from_core(result: CoreTorrentAuthorResult) -> Self {
        let files = result
            .files
            .into_iter()
            .map(|file| TorrentAuthorFileView {
                path: file.path,
                size_bytes: file.size_bytes,
            })
            .collect();
        Self {
            metainfo: general_purpose::STANDARD.encode(result.metainfo),
            magnet_uri: result.magnet_uri,
            info_hash: result.info_hash,
            piece_length: result.piece_length,
            total_size: result.total_size,
            files,
            warnings: result.warnings,
            trackers: result.trackers,
            web_seeds: result.web_seeds,
            private: result.private,
            comment: result.comment,
            source: result.source,
        }
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

/// Body accepted by `PATCH /v1/torrents/{id}/options`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TorrentOptionsRequest {
    #[serde(default)]
    /// Optional per-torrent peer connection cap.
    pub connections_limit: Option<i32>,
    #[serde(default)]
    /// Optional override for peer exchange behaviour.
    pub pex_enabled: Option<bool>,
    #[serde(default)]
    /// Optional comment update for torrent metadata.
    pub comment: Option<String>,
    #[serde(default)]
    /// Optional source update for torrent metadata.
    pub source: Option<String>,
    #[serde(default)]
    /// Optional private flag update for torrent metadata.
    pub private: Option<bool>,
    #[serde(default)]
    /// Optional toggle to pause or resume the torrent.
    pub paused: Option<bool>,
    #[serde(default)]
    /// Optional toggle for super-seeding.
    pub super_seeding: Option<bool>,
    #[serde(default)]
    /// Optional override for auto-managed queueing.
    pub auto_managed: Option<bool>,
    #[serde(default)]
    /// Optional queue position when auto-managed is disabled.
    pub queue_position: Option<i32>,
    #[serde(default)]
    /// Optional share ratio stop threshold.
    pub seed_ratio_limit: Option<f64>,
    #[serde(default)]
    /// Optional seeding time stop threshold in seconds.
    pub seed_time_limit: Option<u64>,
}

impl TorrentOptionsRequest {
    /// Reject unsupported metadata mutations for post-add updates.
    #[must_use]
    pub const fn unsupported_metadata_message(&self) -> Option<&'static str> {
        if self.comment.is_some() {
            return Some("comment updates are not supported post-add");
        }
        if self.source.is_some() {
            return Some("source updates are not supported post-add");
        }
        if self.private.is_some() {
            return Some("private flag updates are not supported post-add");
        }
        None
    }

    /// Reject unsupported per-torrent seeding overrides.
    #[must_use]
    pub const fn unsupported_seed_limit_message(&self) -> Option<&'static str> {
        if self.seed_ratio_limit.is_some() {
            return Some("seed_ratio_limit overrides are not supported per-torrent");
        }
        if self.seed_time_limit.is_some() {
            return Some("seed_time_limit overrides are not supported per-torrent");
        }
        None
    }

    /// Translate the request payload into a domain update.
    #[must_use]
    pub fn to_update(&self) -> TorrentOptionsUpdate {
        TorrentOptionsUpdate {
            connections_limit: self
                .connections_limit
                .and_then(|value| if value > 0 { Some(value) } else { None }),
            pex_enabled: self.pex_enabled,
            comment: self.comment.clone(),
            source: self.source.clone(),
            private: self.private,
            paused: self.paused,
            super_seeding: self.super_seeding,
            auto_managed: self.auto_managed,
            queue_position: self.queue_position,
            seed_ratio_limit: self.seed_ratio_limit,
            seed_time_limit: self.seed_time_limit,
        }
    }

    /// Returns true when no options were provided.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.connections_limit.is_none()
            && self.pex_enabled.is_none()
            && self.comment.is_none()
            && self.source.is_none()
            && self.private.is_none()
            && self.paused.is_none()
            && self.super_seeding.is_none()
            && self.auto_managed.is_none()
            && self.queue_position.is_none()
            && self.seed_ratio_limit.is_none()
            && self.seed_time_limit.is_none()
    }
}

/// Describes a single tracker associated with a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackerView {
    /// Tracker URL.
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional human-readable status (if available from the engine).
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional last message reported by the tracker.
    pub message: Option<String>,
}

/// Response returned by `GET /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentTrackersResponse {
    /// Trackers currently attached to the torrent.
    pub trackers: Vec<TrackerView>,
}

/// Peer snapshot exposed via the torrent endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentPeer {
    /// Endpoint (host:port).
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional client identifier reported by the peer.
    pub client: Option<String>,
    /// Progress fraction (0.0-1.0).
    pub progress: f64,
    /// Current download rate in bytes per second.
    pub download_bps: u64,
    /// Current upload rate in bytes per second.
    pub upload_bps: u64,
    /// Interest flags for the peer connection.
    pub interest: PeerInterest,
    /// Choke flags for the peer connection.
    pub choke: PeerChoke,
}

impl From<PeerSnapshot> for TorrentPeer {
    fn from(peer: PeerSnapshot) -> Self {
        Self {
            endpoint: peer.endpoint,
            client: peer.client,
            progress: peer.progress,
            download_bps: peer.download_bps,
            upload_bps: peer.upload_bps,
            interest: peer.interest,
            choke: peer.choke,
        }
    }
}

/// Body accepted by `DELETE /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentTrackersRemoveRequest {
    #[serde(default)]
    /// Trackers that should be removed from the torrent.
    pub trackers: Vec<String>,
}

/// Body accepted by `PATCH /v1/torrents/{id}/trackers`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentTrackersRequest {
    #[serde(default)]
    /// Trackers to apply.
    pub trackers: Vec<String>,
    #[serde(default)]
    /// Whether to replace all trackers with the supplied set.
    pub replace: bool,
}

impl TorrentTrackersRequest {
    /// Translate into the domain update.
    #[must_use]
    pub const fn to_update(&self, trackers: Vec<String>) -> TorrentTrackersUpdate {
        TorrentTrackersUpdate {
            trackers,
            replace: self.replace,
        }
    }

    /// Returns true when no tracker changes were supplied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.trackers.is_empty()
    }
}

/// Body accepted by `PATCH /v1/torrents/{id}/web_seeds`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TorrentWebSeedsRequest {
    #[serde(default)]
    /// Web seeds to apply.
    pub web_seeds: Vec<String>,
    #[serde(default)]
    /// Whether to replace existing web seeds.
    pub replace: bool,
}

impl TorrentWebSeedsRequest {
    /// Translate into the domain update.
    #[must_use]
    pub const fn to_update(&self, web_seeds: Vec<String>) -> TorrentWebSeedsUpdate {
        TorrentWebSeedsUpdate {
            web_seeds,
            replace: self.replace,
        }
    }

    /// Returns true when no web seeds were supplied.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.web_seeds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use chrono::{TimeZone, Utc};
    use revaer_events::TorrentState;
    use revaer_torrent_core::{
        FilePriority, FilePriorityOverride, StorageMode, TorrentFile, TorrentProgress,
        TorrentRates, TorrentStatus,
    };

    #[test]
    fn torrent_create_request_to_options_maps_fields() {
        let request = TorrentCreateRequest {
            name: Some("Example".to_string()),
            download_dir: Some("/downloads".to_string()),
            sequential: Some(true),
            include: vec!["**/*.mkv".to_string()],
            exclude: vec!["**/*.tmp".to_string()],
            skip_fluff: true,
            max_download_bps: Some(4_096),
            max_upload_bps: Some(2_048),
            max_connections: Some(50),
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(7_200),
            start_paused: Some(true),
            seed_mode: Some(true),
            hash_check_sample_pct: Some(25),
            super_seeding: Some(true),
            tags: vec!["tag-a".to_string(), "tag-b".to_string()],
            auto_managed: Some(false),
            queue_position: Some(2),
            pex_enabled: Some(false),
            web_seeds: vec!["http://seed.example/file".to_string()],
            replace_web_seeds: true,
            storage_mode: Some(StorageMode::Allocate),
            ..TorrentCreateRequest::default()
        };

        let options = request.to_options();
        assert_eq!(options.name_hint.as_deref(), Some("Example"));
        assert_eq!(options.download_dir.as_deref(), Some("/downloads"));
        assert_eq!(options.sequential, Some(true));
        assert_eq!(options.file_rules.include, vec!["**/*.mkv".to_string()]);
        assert_eq!(options.file_rules.exclude, vec!["**/*.tmp".to_string()]);
        assert!(options.file_rules.skip_fluff);
        assert_eq!(options.rate_limit.download_bps, Some(4_096));
        assert_eq!(options.rate_limit.upload_bps, Some(2_048));
        assert_eq!(options.connections_limit, Some(50));
        assert_eq!(options.seed_ratio_limit, Some(1.5));
        assert_eq!(options.seed_time_limit, Some(7_200));
        assert_eq!(options.start_paused, Some(true));
        assert_eq!(options.seed_mode, Some(true));
        assert_eq!(options.hash_check_sample_pct, Some(25));
        assert_eq!(options.super_seeding, Some(true));
        assert_eq!(options.tags, vec!["tag-a".to_string(), "tag-b".to_string()]);
        assert_eq!(options.auto_managed, Some(false));
        assert_eq!(options.queue_position, Some(2));
        assert_eq!(options.pex_enabled, Some(false));
        assert_eq!(options.storage_mode, Some(StorageMode::Allocate));
        assert_eq!(
            options.web_seeds,
            vec!["http://seed.example/file".to_string()]
        );
        assert!(options.replace_web_seeds);
    }

    #[test]
    fn torrent_create_request_to_source_prefers_magnet() {
        let request = TorrentCreateRequest {
            magnet: Some("magnet:?xt=urn:btih:example".to_string()),
            metainfo: Some(general_purpose::STANDARD.encode(b"payload")),
            ..TorrentCreateRequest::default()
        };

        match request.to_source().expect("source present") {
            TorrentSource::Magnet { uri } => {
                assert!(uri.starts_with("magnet:?xt=urn:btih:example"));
            }
            TorrentSource::Metainfo { .. } => panic!("unexpected metainfo source"),
        }
    }

    #[test]
    fn torrent_create_request_to_source_decodes_metainfo() {
        let encoded = general_purpose::STANDARD.encode(b"payload-bytes");
        let request = TorrentCreateRequest {
            metainfo: Some(encoded),
            ..TorrentCreateRequest::default()
        };

        match request.to_source().expect("source present") {
            TorrentSource::Metainfo { bytes } => assert_eq!(bytes, b"payload-bytes"),
            TorrentSource::Magnet { .. } => panic!("unexpected magnet source"),
        }
    }

    #[test]
    fn torrent_create_request_ignores_non_positive_connection_limit() {
        let request = TorrentCreateRequest {
            max_connections: Some(0),
            ..TorrentCreateRequest::default()
        };

        let options = request.to_options();
        assert!(options.connections_limit.is_none());
    }

    #[test]
    fn torrent_summary_and_detail_from_status_preserves_metadata() {
        let torrent_id = Uuid::new_v4();
        let status = TorrentStatus {
            id: torrent_id,
            name: Some("Example Torrent".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 75,
                bytes_total: 100,
                eta_seconds: Some(15),
            },
            rates: TorrentRates {
                download_bps: 1_024,
                upload_bps: 512,
                ratio: 0.5,
            },
            files: Some(vec![TorrentFile {
                index: 0,
                path: "movie.mkv".to_string(),
                size_bytes: 100,
                bytes_completed: 75,
                priority: FilePriority::High,
                selected: true,
            }]),
            library_path: Some("/library/movie".to_string()),
            download_dir: Some("/downloads/movie".to_string()),
            comment: Some("note".to_string()),
            source: Some("source".to_string()),
            private: Some(true),
            sequential: true,
            added_at: Utc.timestamp_millis_opt(0).unwrap(),
            completed_at: Some(Utc.timestamp_millis_opt(1_000).unwrap()),
            last_updated: Utc.timestamp_millis_opt(2_000).unwrap(),
        };

        let summary = TorrentSummary::from(status.clone()).with_metadata(
            vec!["tag".to_string()],
            Some("movies".to_string()),
            vec!["tracker".to_string()],
            Some(revaer_torrent_core::TorrentRateLimit {
                download_bps: Some(5_000),
                upload_bps: None,
            }),
            Some(80),
        );
        assert_eq!(summary.id, torrent_id);
        assert_eq!(summary.state.kind, TorrentStateKind::Completed);
        assert_eq!(summary.tags, vec!["tag".to_string()]);
        assert_eq!(summary.category.as_deref(), Some("movies"));
        assert_eq!(summary.trackers, vec!["tracker".to_string()]);
        assert_eq!(
            summary.rate_limit.and_then(|limit| limit.download_bps),
            Some(5_000)
        );
        assert_eq!(summary.connections_limit, Some(80));

        let detail = TorrentDetail::from(status);
        assert_eq!(detail.summary.id, torrent_id);
        let settings = detail.settings.expect("settings should be present");
        assert_eq!(settings.download_dir.as_deref(), Some("/downloads/movie"));
        assert!(settings.sequential);
        assert!(settings.selection.is_none());
        let files = detail.files.expect("files should be present");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "movie.mkv");
        assert!(files[0].selected);
    }

    #[test]
    fn torrent_selection_request_converts_to_update() {
        let request = TorrentSelectionRequest {
            include: vec!["**/*.mkv".to_string()],
            exclude: vec!["**/*.tmp".to_string()],
            skip_fluff: Some(true),
            priorities: vec![FilePriorityOverride {
                index: 1,
                priority: FilePriority::Low,
            }],
        };

        let update: FileSelectionUpdate = request.clone().into();
        assert_eq!(update.include, request.include);
        assert_eq!(update.exclude, request.exclude);
        assert!(update.skip_fluff);
        assert_eq!(update.priorities.len(), 1);
        assert_eq!(update.priorities[0].index, 1);
        assert_eq!(update.priorities[0].priority, FilePriority::Low);
    }

    #[test]
    fn torrent_options_request_to_update_filters_values() {
        let request = TorrentOptionsRequest {
            connections_limit: Some(0),
            pex_enabled: Some(false),
            comment: None,
            source: None,
            private: None,
            paused: Some(true),
            super_seeding: Some(true),
            auto_managed: Some(false),
            queue_position: Some(3),
            seed_ratio_limit: Some(2.0),
            seed_time_limit: Some(3_600),
        };

        let update = request.to_update();
        assert!(update.connections_limit.is_none());
        assert_eq!(update.pex_enabled, Some(false));
        assert_eq!(update.paused, Some(true));
        assert_eq!(update.super_seeding, Some(true));
        assert_eq!(update.auto_managed, Some(false));
        assert_eq!(update.queue_position, Some(3));
        assert_eq!(update.seed_ratio_limit, Some(2.0));
        assert_eq!(update.seed_time_limit, Some(3_600));
        assert!(!request.is_empty());
    }

    #[test]
    fn torrent_trackers_request_to_update_applies_replace_flag() {
        let request = TorrentTrackersRequest {
            trackers: vec!["https://tracker.example/announce".to_string()],
            replace: true,
        };

        let update = request.to_update(request.trackers.clone());
        assert_eq!(
            update.trackers,
            vec!["https://tracker.example/announce".to_string()]
        );
        assert!(update.replace);
        assert!(!request.is_empty());
    }

    #[test]
    fn torrent_web_seeds_request_to_update_applies_replace_flag() {
        let request = TorrentWebSeedsRequest {
            web_seeds: vec!["http://seed.example/file".to_string()],
            replace: false,
        };

        let update = request.to_update(request.web_seeds.clone());
        assert_eq!(
            update.web_seeds,
            vec!["http://seed.example/file".to_string()]
        );
        assert!(!update.replace);
        assert!(!request.is_empty());
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
    /// Relocate torrent storage to a new download directory.
    Move {
        /// Destination path for in-progress data.
        download_dir: String,
    },
    /// Set or clear a deadline for a specific piece to support streaming.
    PieceDeadline {
        /// Zero-based piece index to target.
        piece: u32,
        #[serde(default)]
        /// Deadline in milliseconds; when omitted the deadline is cleared.
        deadline_ms: Option<u32>,
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
