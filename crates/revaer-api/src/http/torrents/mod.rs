//! Torrent HTTP helpers (pagination, metadata composition, filters).

use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::http::errors::ApiError;
use crate::models::{TorrentDetail, TorrentStateKind, TorrentSummary};
use revaer_torrent_core::{TorrentInspector, TorrentStatus, TorrentWorkflow};

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
}

impl TorrentMetadata {
    #[must_use]
    pub(crate) const fn new(tags: Vec<String>, trackers: Vec<String>) -> Self {
        Self { tags, trackers }
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
    TorrentSummary::from(status).with_metadata(metadata.tags, metadata.trackers)
}

#[must_use]
pub(crate) fn detail_from_components(
    status: TorrentStatus,
    metadata: TorrentMetadata,
) -> TorrentDetail {
    let mut detail = TorrentDetail::from(status);
    detail.summary = detail
        .summary
        .with_metadata(metadata.tags, metadata.trackers);
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

#[must_use]
pub(crate) fn normalise_lower(value: &str) -> String {
    value.trim().to_lowercase()
}
