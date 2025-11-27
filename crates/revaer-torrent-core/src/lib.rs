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
#![allow(clippy::module_name_repetitions)]
#![allow(unexpected_cfgs)]
#![allow(clippy::multiple_crate_versions)]

//! Engine-agnostic torrent interfaces and DTOs shared across the workspace.

use anyhow::bail;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Source describing how a torrent should be added to the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TorrentSource {
    /// Represents a magnet URI that should be fetched.
    Magnet {
        /// Magnet URI to resolve and add.
        uri: String,
    },
    /// Represents raw `.torrent` metainfo bytes.
    Metainfo {
        /// Bencoded metainfo payload.
        bytes: Vec<u8>,
    },
}

impl TorrentSource {
    #[must_use]
    /// Convenience constructor for magnet-based sources.
    pub fn magnet(uri: impl Into<String>) -> Self {
        Self::Magnet { uri: uri.into() }
    }

    #[must_use]
    /// Convenience constructor for metainfo-based sources.
    pub fn metainfo(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Metainfo {
            bytes: bytes.into(),
        }
    }
}

/// Request payload for admitting a torrent into the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTorrent {
    /// Unique identifier assigned by the caller.
    pub id: Uuid,
    /// How the torrent should be retrieved (magnet or metainfo).
    pub source: TorrentSource,
    #[serde(default)]
    /// Optional knobs applied alongside admission.
    pub options: AddTorrentOptions,
}

/// Optional knobs that accompany a torrent admission request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddTorrentOptions {
    /// Friendly name to display before metadata is fetched.
    pub name_hint: Option<String>,
    /// Optional override for the download root within the engine profile.
    pub download_dir: Option<String>,
    /// When provided, forces the initial sequential download strategy.
    pub sequential: Option<bool>,
    /// Pre-configured file selection rules.
    #[serde(default)]
    pub file_rules: FileSelectionRules,
    /// Per-torrent rate limits applied immediately after the torrent is added.
    #[serde(default)]
    pub rate_limit: TorrentRateLimit,
    /// Arbitrary labels propagated to downstream consumers.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Per-torrent rate limiting knobs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentRateLimit {
    /// Maximum download rate in bytes per second.
    pub download_bps: Option<u64>,
    /// Maximum upload rate in bytes per second.
    pub upload_bps: Option<u64>,
}

/// Selection rules applied to the torrent's file set after metadata discovery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileSelectionRules {
    #[serde(default)]
    /// Glob-style patterns that force inclusion.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob-style patterns that force exclusion.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Drop known "fluff" files from selection.
    pub skip_fluff: bool,
}

/// Request payload for updating an existing torrent's file selection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileSelectionUpdate {
    #[serde(default)]
    /// Glob-style patterns that force inclusion.
    pub include: Vec<String>,
    #[serde(default)]
    /// Glob-style patterns that force exclusion.
    pub exclude: Vec<String>,
    #[serde(default)]
    /// Drop known "fluff" files from selection.
    pub skip_fluff: bool,
    #[serde(default)]
    /// File priority overrides to apply post-selection.
    pub priorities: Vec<FilePriorityOverride>,
}

/// Per-file priority override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePriorityOverride {
    /// File index within the torrent payload.
    pub index: u32,
    /// Desired priority for the file.
    pub priority: FilePriority,
}

/// Priority level recognized by libtorrent.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilePriority {
    /// Do not download the file.
    Skip,
    /// Throttle the download priority.
    Low,
    /// Default priority level assigned by the engine.
    #[default]
    Normal,
    /// Highest available priority for urgent files.
    High,
}

/// Options controlling how the engine removes torrents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct RemoveTorrent {
    #[serde(default)]
    /// Whether to remove on-disk data alongside the torrent metadata.
    pub with_data: bool,
}

/// Lightweight transfer statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentRates {
    #[serde(default)]
    /// Current download rate in bytes per second.
    pub download_bps: u64,
    #[serde(default)]
    /// Current upload rate in bytes per second.
    pub upload_bps: u64,
    #[serde(default)]
    /// Share ratio (uploaded/downloaded) reported by the engine.
    pub ratio: f64,
}

/// Aggregated progress metrics for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentProgress {
    /// Total bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes expected for completion.
    pub bytes_total: u64,
    #[serde(default)]
    /// Estimated time remaining for completion in seconds.
    pub eta_seconds: Option<u64>,
}

impl TorrentProgress {
    #[must_use]
    /// Calculate the completion percentage (0-100).
    pub fn percent_complete(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (to_f64(self.bytes_downloaded) / to_f64(self.bytes_total)) * 100.0
        }
    }
}

const fn to_f64(value: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 to f64 conversion is required for user-facing percentage reporting"
    )]
    {
        value as f64
    }
}

/// Individual file exposed by a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    /// Index of the file within the torrent metainfo.
    pub index: u32,
    /// Relative path of the file within the torrent payload.
    pub path: String,
    /// Total size of the file in bytes.
    pub size_bytes: u64,
    /// Bytes downloaded so far for this file.
    pub bytes_completed: u64,
    /// Current priority level.
    pub priority: FilePriority,
    /// Whether the file is selected for download.
    pub selected: bool,
}

/// High-level torrent status surfaced by the inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentStatus {
    /// Identifier for the torrent.
    pub id: Uuid,
    /// Optional human-readable name for the torrent.
    pub name: Option<String>,
    /// Current lifecycle state.
    pub state: revaer_events::TorrentState,
    /// Progress snapshot associated with the torrent.
    pub progress: TorrentProgress,
    /// Transfer rates associated with the torrent.
    pub rates: TorrentRates,
    /// Optional collection of files (when metadata is available).
    pub files: Option<Vec<TorrentFile>>,
    /// Library path populated when the torrent is completed.
    pub library_path: Option<String>,
    /// Download directory assigned to the torrent.
    pub download_dir: Option<String>,
    /// Whether sequential mode is active.
    pub sequential: bool,
    /// Timestamp when the torrent was added.
    pub added_at: DateTime<Utc>,
    /// Timestamp when the torrent completed, if available.
    pub completed_at: Option<DateTime<Utc>>,
    /// Timestamp of the last status update.
    pub last_updated: DateTime<Utc>,
}

impl Default for TorrentStatus {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            name: None,
            state: revaer_events::TorrentState::Queued,
            progress: TorrentProgress::default(),
            rates: TorrentRates::default(),
            files: None,
            library_path: None,
            download_dir: None,
            sequential: false,
            added_at: Utc::now(),
            completed_at: None,
            last_updated: Utc::now(),
        }
    }
}

/// Events emitted by the torrent engine task before they are translated into the shared event bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    /// File metadata became available.
    FilesDiscovered {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Discovered file listing.
        files: Vec<TorrentFile>,
    },
    /// Progress metrics were updated.
    Progress {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Updated progress snapshot.
        progress: TorrentProgress,
        /// Updated transfer rates.
        rates: TorrentRates,
    },
    /// Torrent state transitioned.
    StateChanged {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Updated lifecycle state.
        state: revaer_events::TorrentState,
    },
    /// Torrent completed and produced a library artifact.
    Completed {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Path to the completed artifact.
        library_path: String,
    },
    /// Torrent metadata (name/path) changed.
    MetadataUpdated {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Optional updated display name.
        name: Option<String>,
        /// Optional updated download directory.
        download_dir: Option<String>,
    },
    /// Resume data became available.
    ResumeData {
        /// Torrent identifier.
        torrent_id: Uuid,
        /// Raw resume data payload.
        payload: Vec<u8>,
    },
    /// Engine reported an error condition.
    Error {
        /// Torrent identifier associated with the error.
        torrent_id: Uuid,
        /// Human-readable failure description.
        message: String,
    },
}

/// Primary engine trait implemented by adapters (e.g. libtorrent).
#[async_trait]
pub trait TorrentEngine: Send + Sync {
    /// Admit a new torrent into the underlying engine.
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    /// Remove a torrent from the engine, optionally deleting data.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported by this engine");
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported by this engine");
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported by this engine");
    }

    /// Update per-torrent or global rate limits.
    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported by this engine");
    }

    /// Adjust the file selection for a torrent.
    /// Adjust the file selection; default implementation reports lack of support.
    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported by this engine");
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported by this engine");
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported by this engine");
    }
}

/// Workflow façade exposed to the API layer for torrent lifecycle control.
#[async_trait]
pub trait TorrentWorkflow: Send + Sync {
    /// Admit a new torrent via the workflow façade.
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    /// Remove a torrent via the workflow façade.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported");
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported");
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported");
    }

    /// Update per-torrent or global rate limits via the workflow façade.
    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported");
    }

    /// Adjust the file selection via the workflow façade.
    /// Default implementation reports lack of support.
    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported");
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported");
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported");
    }
}

/// Inspector trait used by API consumers to fetch torrent snapshots.
#[async_trait]
pub trait TorrentInspector: Send + Sync {
    /// Retrieve the full torrent status list.
    async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>>;

    /// Retrieve an individual torrent status snapshot.
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[test]
    fn torrent_source_helpers_construct_variants() {
        let magnet = TorrentSource::magnet("magnet:?xt=urn:btih:demo");
        match magnet {
            TorrentSource::Magnet { uri } => assert!(uri.contains("demo")),
            TorrentSource::Metainfo { .. } => panic!("expected magnet variant"),
        }

        let data = vec![1_u8, 2, 3];
        let meta = TorrentSource::metainfo(data.clone());
        match meta {
            TorrentSource::Metainfo { bytes } => assert_eq!(bytes, data),
            TorrentSource::Magnet { .. } => panic!("expected metainfo variant"),
        }
    }

    #[test]
    fn progress_percent_handles_zero_total() {
        let zero = TorrentProgress {
            bytes_downloaded: 0,
            bytes_total: 0,
            eta_seconds: None,
        };
        assert!(zero.percent_complete().abs() < f64::EPSILON);

        let half = TorrentProgress {
            bytes_downloaded: 5,
            bytes_total: 10,
            eta_seconds: None,
        };
        assert!((half.percent_complete() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn torrent_status_default_sets_reasonable_fields() {
        let status = TorrentStatus::default();
        assert_eq!(status.state, revaer_events::TorrentState::Queued);
        assert_eq!(status.progress.bytes_downloaded, 0);
        assert!(!status.sequential);
    }

    struct StubEngine;

    #[async_trait]
    impl TorrentEngine for StubEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn engine_default_methods_error() {
        let engine = StubEngine;
        let id = Uuid::new_v4();

        let pause = engine.pause_torrent(id).await;
        assert!(pause.is_err(), "pause should not be supported");

        let resume = engine.resume_torrent(id).await;
        assert!(resume.is_err(), "resume should not be supported");

        let sequential = engine.set_sequential(id, true).await;
        assert!(
            sequential.is_err(),
            "sequential toggle should not be supported"
        );
    }

    #[tokio::test]
    async fn engine_update_and_reannounce_errors() {
        let engine = StubEngine;
        let id = Uuid::new_v4();
        assert!(
            engine
                .update_limits(Some(id), TorrentRateLimit::default())
                .await
                .is_err()
        );
        assert!(
            engine
                .update_selection(id, FileSelectionUpdate::default())
                .await
                .is_err()
        );
        assert!(engine.reannounce(id).await.is_err());
        assert!(engine.recheck(id).await.is_err());
    }

    struct StubWorkflow;

    #[async_trait]
    impl TorrentWorkflow for StubWorkflow {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn workflow_default_methods_error() {
        let workflow = StubWorkflow;
        let id = Uuid::new_v4();

        assert!(workflow.pause_torrent(id).await.is_err());
        assert!(workflow.resume_torrent(id).await.is_err());
        assert!(
            workflow
                .set_sequential(id, true)
                .await
                .expect_err("sequential should error")
                .to_string()
                .contains("not supported")
        );
    }

    #[tokio::test]
    async fn workflow_update_methods_error() {
        let workflow = StubWorkflow;
        let id = Uuid::new_v4();
        assert!(
            workflow
                .update_limits(Some(id), TorrentRateLimit::default())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_selection(id, FileSelectionUpdate::default())
                .await
                .is_err()
        );
        assert!(workflow.reannounce(id).await.is_err());
        assert!(workflow.recheck(id).await.is_err());
    }
}
