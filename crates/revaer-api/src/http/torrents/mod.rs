//! Torrent HTTP helpers (pagination, metadata composition, filters).

use std::collections::HashSet;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::http::errors::ApiError;
use crate::models::{TorrentDetail, TorrentSelectionView, TorrentStateKind, TorrentSummary};
use revaer_torrent_core::{
    FileSelectionUpdate, TorrentInspector, TorrentRateLimit, TorrentStatus, TorrentWorkflow,
};

pub mod handlers;

/// Handle pair that exposes torrent workflow and inspection capabilities to the
/// HTTP layer.
#[derive(Clone)]
pub struct TorrentHandles {
    workflow: Arc<dyn TorrentWorkflow>,
    inspector: Arc<dyn TorrentInspector>,
}

impl TorrentHandles {
    /// Construct a new handle pair from shared workflow and inspector traits.
    #[must_use]
    pub fn new(workflow: Arc<dyn TorrentWorkflow>, inspector: Arc<dyn TorrentInspector>) -> Self {
        Self {
            workflow,
            inspector,
        }
    }

    #[must_use]
    /// Accessor for the torrent workflow implementation.
    pub fn workflow(&self) -> &Arc<dyn TorrentWorkflow> {
        &self.workflow
    }

    #[must_use]
    /// Accessor for the torrent inspector implementation.
    pub fn inspector(&self) -> &Arc<dyn TorrentInspector> {
        &self.inspector
    }
}

/// Tags/trackers metadata captured alongside torrent status snapshots.
#[derive(Clone, Debug, Default)]
pub(crate) struct TorrentMetadata {
    pub(crate) tags: Vec<String>,
    pub(crate) trackers: Vec<String>,
    pub(crate) tracker_messages: std::collections::HashMap<String, String>,
    pub(crate) web_seeds: Vec<String>,
    pub(crate) rate_limit: Option<TorrentRateLimit>,
    pub(crate) connections_limit: Option<i32>,
    pub(crate) selection: FileSelectionUpdate,
    pub(crate) super_seeding: Option<bool>,
    pub(crate) seed_mode: Option<bool>,
    pub(crate) download_dir: Option<String>,
    pub(crate) seed_ratio_limit: Option<f64>,
    pub(crate) seed_time_limit: Option<u64>,
    pub(crate) auto_managed: Option<bool>,
    pub(crate) queue_position: Option<i32>,
    pub(crate) pex_enabled: Option<bool>,
    pub(crate) replace_trackers: bool,
    pub(crate) replace_web_seeds: bool,
}

impl TorrentMetadata {
    #[must_use]
    pub(crate) fn new(
        tags: Vec<String>,
        trackers: Vec<String>,
        web_seeds: Vec<String>,
        rate_limit: Option<TorrentRateLimit>,
        connections_limit: Option<i32>,
        selection: FileSelectionUpdate,
        download_dir: Option<String>,
    ) -> Self {
        Self {
            tags,
            trackers,
            tracker_messages: std::collections::HashMap::new(),
            web_seeds,
            rate_limit,
            connections_limit,
            selection,
            super_seeding: None,
            seed_mode: None,
            download_dir,
            seed_ratio_limit: None,
            seed_time_limit: None,
            auto_managed: None,
            queue_position: None,
            pex_enabled: None,
            replace_trackers: false,
            replace_web_seeds: false,
        }
    }

    #[must_use]
    pub(crate) fn from_request(
        request: &crate::models::TorrentCreateRequest,
        trackers: Vec<String>,
        web_seeds: Vec<String>,
    ) -> Self {
        let rate_limit = rate_limit_from_limits(request.max_download_bps, request.max_upload_bps);
        let connections_limit = request.max_connections.filter(|value| *value > 0);
        let selection = FileSelectionUpdate {
            include: request.include.clone(),
            exclude: request.exclude.clone(),
            skip_fluff: request.skip_fluff,
            priorities: Vec::new(),
        };
        Self::new(
            request.tags.clone(),
            trackers,
            web_seeds,
            rate_limit,
            connections_limit,
            selection,
            request.download_dir.clone(),
        )
        .with_additional_flags(request)
    }

    const fn with_additional_flags(
        mut self,
        request: &crate::models::TorrentCreateRequest,
    ) -> Self {
        self.super_seeding = request.super_seeding;
        self.seed_mode = request.seed_mode;
        self.seed_ratio_limit = request.seed_ratio_limit;
        self.seed_time_limit = request.seed_time_limit;
        self.auto_managed = request.auto_managed;
        self.queue_position = request.queue_position;
        self.pex_enabled = request.pex_enabled;
        self.replace_trackers = request.replace_trackers;
        self.replace_web_seeds = request.replace_web_seeds;
        self
    }

    pub(crate) const fn apply_rate_limit(&mut self, rate_limit: &TorrentRateLimit) {
        self.rate_limit = rate_limit_from_limits(rate_limit.download_bps, rate_limit.upload_bps);
    }
}

/// Query string parameters for torrent list endpoints.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct TorrentListQuery {
    #[serde(default)]
    pub(crate) limit: Option<u32>,
    #[serde(default)]
    pub(crate) cursor: Option<String>,
    #[serde(default)]
    pub(crate) state: Option<String>,
    #[serde(default)]
    pub(crate) tracker: Option<String>,
    #[serde(default)]
    pub(crate) extension: Option<String>,
    #[serde(default)]
    pub(crate) tags: Option<String>,
    #[serde(default)]
    pub(crate) name: Option<String>,
}

/// Combined status/metadata entry used for pagination cursors.
#[derive(Clone, Debug)]
pub(crate) struct StatusEntry {
    pub(crate) status: TorrentStatus,
    pub(crate) metadata: TorrentMetadata,
}

/// Cursor token materialised from [`StatusEntry`] positions.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CursorToken {
    pub(crate) last_updated: DateTime<Utc>,
    pub(crate) id: Uuid,
}

#[must_use]
pub(crate) fn summary_from_components(
    status: TorrentStatus,
    metadata: TorrentMetadata,
) -> TorrentSummary {
    TorrentSummary::from(status).with_metadata(
        metadata.tags,
        metadata.trackers,
        metadata.rate_limit,
        metadata.connections_limit,
    )
}

#[must_use]
pub(crate) fn detail_from_components(
    status: TorrentStatus,
    metadata: TorrentMetadata,
) -> TorrentDetail {
    let mut detail = TorrentDetail::from(status);
    let TorrentMetadata {
        tags,
        trackers,
        web_seeds,
        rate_limit,
        connections_limit,
        selection,
        tracker_messages,
        super_seeding,
        seed_mode,
        seed_ratio_limit,
        seed_time_limit,
        auto_managed,
        queue_position,
        pex_enabled,
        replace_trackers: _,
        replace_web_seeds: _,
        ..
    } = metadata;
    detail.summary = detail.summary.with_metadata(
        tags.clone(),
        trackers.clone(),
        rate_limit.clone(),
        connections_limit,
    );
    if let Some(settings) = detail.settings.as_mut() {
        settings.tags = tags;
        settings.trackers = trackers;
        settings.tracker_messages = tracker_messages;
        settings.rate_limit = rate_limit;
        settings.connections_limit = connections_limit;
        settings.selection = Some(TorrentSelectionView::from(&selection));
        settings.super_seeding = super_seeding;
        settings.seed_mode = seed_mode;
        settings.seed_ratio_limit = seed_ratio_limit;
        settings.seed_time_limit = seed_time_limit;
        settings.auto_managed = auto_managed;
        settings.queue_position = queue_position;
        settings.pex_enabled = pex_enabled;
        settings.web_seeds = web_seeds;
    }
    detail
}

pub(crate) fn encode_cursor_from_entry(entry: &StatusEntry) -> Result<String, ApiError> {
    let token = CursorToken {
        last_updated: entry.status.last_updated,
        id: entry.status.id,
    };
    let json = serde_json::to_vec(&token).map_err(|err| {
        tracing::error!(error = %err, "failed to serialise cursor token");
        ApiError::internal("failed to encode pagination cursor")
    })?;
    Ok(general_purpose::STANDARD.encode(json))
}

pub(crate) fn decode_cursor_token(value: &str) -> Result<CursorToken, ApiError> {
    let bytes = general_purpose::STANDARD
        .decode(value)
        .map_err(|_| ApiError::bad_request("cursor token was not valid base64"))?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::bad_request("cursor token malformed"))
}

pub(crate) fn parse_state_filter(value: &str) -> Result<TorrentStateKind, ApiError> {
    match value {
        "queued" => Ok(TorrentStateKind::Queued),
        "fetching_metadata" => Ok(TorrentStateKind::FetchingMetadata),
        "downloading" => Ok(TorrentStateKind::Downloading),
        "seeding" => Ok(TorrentStateKind::Seeding),
        "completed" => Ok(TorrentStateKind::Completed),
        "failed" => Ok(TorrentStateKind::Failed),
        "stopped" => Ok(TorrentStateKind::Stopped),
        other => Err(ApiError::bad_request(format!(
            "state filter '{other}' is not recognised"
        ))),
    }
}

pub(crate) fn split_comma_separated(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

pub(crate) fn normalize_trackers(inputs: &[String]) -> Result<Vec<String>, ApiError> {
    let mut seen = HashSet::new();
    let mut trackers = Vec::new();
    for raw in inputs {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let url =
            Url::parse(trimmed).map_err(|_| ApiError::bad_request("tracker URL is malformed"))?;
        match url.scheme() {
            "http" | "https" | "udp" => {}
            other => {
                return Err(ApiError::bad_request(format!(
                    "tracker scheme '{other}' is not supported (http/https/udp only)"
                )));
            }
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.insert(key) {
            trackers.push(trimmed.to_string());
        }
    }
    Ok(trackers)
}

/// Validate and normalise web seed URLs.
pub(crate) fn normalize_web_seeds(inputs: &[String]) -> Result<Vec<String>, ApiError> {
    let mut seen = HashSet::new();
    let mut seeds = Vec::new();
    for seed in inputs {
        let trimmed = seed.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed =
            Url::parse(trimmed).map_err(|_| ApiError::bad_request("web seed URL is not valid"))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(ApiError::bad_request(
                "web seed URLs must use http or https",
            ));
        }
        let normalized = parsed.to_string();
        let key = normalized
            .strip_prefix("http://")
            .or_else(|| normalized.strip_prefix("https://"))
            .unwrap_or(&normalized)
            .to_lowercase();
        if seen.insert(key) {
            seeds.push(normalized);
        }
    }
    Ok(seeds)
}

#[must_use]
pub(crate) fn normalise_lower(value: &str) -> String {
    value.trim().to_lowercase()
}

#[must_use]
pub(crate) const fn rate_limit_from_limits(
    download_bps: Option<u64>,
    upload_bps: Option<u64>,
) -> Option<TorrentRateLimit> {
    match (download_bps, upload_bps) {
        (None, None) => None,
        (download_bps, upload_bps) => Some(TorrentRateLimit {
            download_bps,
            upload_bps,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use revaer_events::TorrentState;
    use revaer_torrent_core::{TorrentProgress, TorrentRates};

    #[test]
    fn detail_carries_metadata_and_selection() {
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("demo".into()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 5,
                bytes_total: 10,
                eta_seconds: Some(1),
            },
            rates: TorrentRates {
                download_bps: 0,
                upload_bps: 0,
                ratio: 0.0,
            },
            files: None,
            library_path: None,
            download_dir: Some("/downloads/demo".into()),
            sequential: true,
            added_at: Utc.timestamp_millis_opt(0).unwrap(),
            completed_at: None,
            last_updated: Utc.timestamp_millis_opt(0).unwrap(),
        };
        let metadata = TorrentMetadata::new(
            vec!["tagA".to_string()],
            vec!["https://tracker.example/announce".to_string()],
            Vec::new(),
            Some(TorrentRateLimit {
                download_bps: Some(1_024),
                upload_bps: None,
            }),
            None,
            FileSelectionUpdate {
                include: vec!["**/*.mkv".to_string()],
                exclude: Vec::new(),
                skip_fluff: true,
                priorities: Vec::new(),
            },
            status.download_dir.clone(),
        );

        let detail = detail_from_components(status, metadata);
        assert_eq!(detail.summary.tags, vec!["tagA".to_string()]);
        assert_eq!(
            detail
                .summary
                .rate_limit
                .as_ref()
                .and_then(|limit| limit.download_bps),
            Some(1_024)
        );

        let settings = detail.settings.expect("settings should be present");
        assert_eq!(settings.tags, vec!["tagA".to_string()]);
        assert_eq!(settings.trackers, vec!["https://tracker.example/announce"]);
        assert_eq!(
            settings
                .selection
                .as_ref()
                .expect("selection present")
                .include,
            vec!["**/*.mkv".to_string()]
        );
        assert!(
            settings
                .selection
                .as_ref()
                .expect("selection present")
                .skip_fluff
        );
        assert!(settings.sequential);
        assert_eq!(settings.download_dir.as_deref(), Some("/downloads/demo"));
    }

    #[test]
    fn metadata_from_request_tracks_selection_and_limits() {
        let request = crate::models::TorrentCreateRequest {
            tags: vec!["demo".to_string()],
            trackers: vec!["https://tracker.example".to_string()],
            include: vec!["**/*.mkv".to_string()],
            exclude: vec!["**/*.tmp".to_string()],
            skip_fluff: true,
            max_download_bps: Some(9_001),
            max_upload_bps: Some(2_048),
            ..Default::default()
        };

        let trackers =
            normalize_trackers(&request.trackers).expect("trackers should normalise for test");
        let mut metadata =
            TorrentMetadata::from_request(&request, trackers, request.web_seeds.clone());
        assert_eq!(metadata.tags, vec!["demo".to_string()]);
        assert_eq!(
            metadata.trackers,
            vec!["https://tracker.example".to_string()]
        );
        assert!(metadata.selection.skip_fluff);
        assert_eq!(metadata.selection.include, vec!["**/*.mkv".to_string()]);
        assert_eq!(metadata.selection.exclude, vec!["**/*.tmp".to_string()]);
        assert_eq!(
            metadata
                .rate_limit
                .as_ref()
                .and_then(|limit| limit.download_bps),
            Some(9_001)
        );

        let cleared = TorrentRateLimit {
            download_bps: None,
            upload_bps: None,
        };
        metadata.apply_rate_limit(&cleared);
        assert!(metadata.rate_limit.is_none());
    }

    #[test]
    fn normalize_trackers_validates_and_deduplicates() {
        let inputs = vec![
            " https://Tracker.Example/announce ".to_string(),
            "udp://tracker.example/announce".to_string(),
            "https://tracker.example/announce".to_string(),
        ];
        let trackers = normalize_trackers(&inputs).expect("normalization should succeed");
        assert_eq!(
            trackers,
            vec![
                "https://Tracker.Example/announce".to_string(),
                "udp://tracker.example/announce".to_string()
            ]
        );
    }

    #[test]
    fn normalize_trackers_rejects_unknown_schemes() {
        let inputs = vec!["ftp://tracker.example/announce".to_string()];
        let err = normalize_trackers(&inputs).expect_err("ftp scheme should be rejected");
        assert!(
            format!("{err:?}").contains("ftp"),
            "expected error to mention unsupported scheme"
        );
    }

    #[test]
    fn normalize_web_seeds_validates_and_deduplicates() {
        let inputs = vec![
            " http://seed.example/path ".to_string(),
            "https://seed.example/path".to_string(),
        ];
        let seeds = normalize_web_seeds(&inputs).expect("web seed normalisation should succeed");
        assert_eq!(seeds, vec!["http://seed.example/path".to_string()]);
    }

    #[test]
    fn normalize_web_seeds_rejects_unknown_scheme() {
        let inputs = vec!["ftp://seed.example/file".to_string()];
        let err = normalize_web_seeds(&inputs).expect_err("ftp should be rejected");
        assert!(format!("{err:?}").contains("http or https"));
    }
}
