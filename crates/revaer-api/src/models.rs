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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TorrentSettingsView {
    #[serde(default)]
    /// Tags associated with the torrent.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Trackers recorded for the torrent.
    pub trackers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent bandwidth limits when present.
    pub rate_limit: Option<TorrentRateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Per-torrent peer connection cap when configured.
    pub connections_limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Download directory applied at admission time.
    pub download_dir: Option<String>,
    /// Whether sequential mode is currently active.
    pub sequential: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// File selection rules most recently requested.
    pub selection: Option<TorrentSelectionView>,
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
        trackers: Vec<String>,
        rate_limit: Option<revaer_torrent_core::TorrentRateLimit>,
        connections_limit: Option<i32>,
    ) -> Self {
        self.tags = tags;
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
            trackers: Vec::new(),
            rate_limit: None,
            connections_limit: None,
            download_dir: status.download_dir.clone(),
            sequential: status.sequential,
            selection: None,
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
    /// Adds the torrent in a paused/queued state when true.
    pub start_paused: Option<bool>,
    #[serde(default)]
    /// Tags to associate with the torrent immediately.
    pub tags: Vec<String>,
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
}

impl TorrentCreateRequest {
    /// Translate the client payload into the engine-facing [`AddTorrentOptions`].
    #[must_use]
    pub fn to_options(&self) -> AddTorrentOptions {
        AddTorrentOptions {
            name_hint: self.name.clone(),
            download_dir: self.download_dir.clone(),
            sequential: self.sequential,
            start_paused: self.start_paused,
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
            tags: self.tags.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use chrono::{TimeZone, Utc};
    use revaer_events::TorrentState;
    use revaer_torrent_core::{
        FilePriority, FilePriorityOverride, TorrentFile, TorrentProgress, TorrentRates,
        TorrentStatus,
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
            start_paused: Some(true),
            tags: vec!["tag-a".to_string(), "tag-b".to_string()],
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
        assert_eq!(options.start_paused, Some(true));
        assert_eq!(options.tags, vec!["tag-a".to_string(), "tag-b".to_string()]);
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
            sequential: true,
            added_at: Utc.timestamp_millis_opt(0).unwrap(),
            completed_at: Some(Utc.timestamp_millis_opt(1_000).unwrap()),
            last_updated: Utc.timestamp_millis_opt(2_000).unwrap(),
        };

        let summary = TorrentSummary::from(status.clone()).with_metadata(
            vec!["tag".to_string()],
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
