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
    pub(crate) rate_limit: Option<TorrentRateLimit>,
    pub(crate) selection: FileSelectionUpdate,
}

impl TorrentMetadata {
    #[must_use]
    pub(crate) const fn new(
        tags: Vec<String>,
        trackers: Vec<String>,
        rate_limit: Option<TorrentRateLimit>,
        selection: FileSelectionUpdate,
    ) -> Self {
        Self {
            tags,
            trackers,
            rate_limit,
            selection,
        }
    }

    #[must_use]
    pub(crate) fn from_request(
        request: &crate::models::TorrentCreateRequest,
        trackers: Vec<String>,
    ) -> Self {
        let rate_limit = rate_limit_from_limits(request.max_download_bps, request.max_upload_bps);
        let selection = FileSelectionUpdate {
            include: request.include.clone(),
            exclude: request.exclude.clone(),
            skip_fluff: request.skip_fluff,
            priorities: Vec::new(),
        };
        Self::new(request.tags.clone(), trackers, rate_limit, selection)
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
        rate_limit,
        selection,
    } = metadata;
    detail.summary =
        detail
            .summary
            .with_metadata(tags.clone(), trackers.clone(), rate_limit.clone());
    if let Some(settings) = detail.settings.as_mut() {
        settings.tags = tags;
        settings.trackers = trackers;
        settings.rate_limit = rate_limit;
        settings.selection = Some(TorrentSelectionView::from(&selection));
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
            Some(TorrentRateLimit {
                download_bps: Some(1_024),
                upload_bps: None,
            }),
            FileSelectionUpdate {
                include: vec!["**/*.mkv".to_string()],
                exclude: Vec::new(),
                skip_fluff: true,
                priorities: Vec::new(),
            },
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
        let mut metadata = TorrentMetadata::from_request(&request, trackers);
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
}
