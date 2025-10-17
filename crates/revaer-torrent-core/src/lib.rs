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
    Magnet { uri: String },
    Metainfo { bytes: Vec<u8> },
}

impl TorrentSource {
    #[must_use]
    pub fn magnet(uri: impl Into<String>) -> Self {
        Self::Magnet { uri: uri.into() }
    }

    #[must_use]
    pub fn metainfo(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Metainfo {
            bytes: bytes.into(),
        }
    }
}

/// Request payload for admitting a torrent into the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTorrent {
    pub id: Uuid,
    pub source: TorrentSource,
    #[serde(default)]
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
    pub download_bps: Option<u64>,
    pub upload_bps: Option<u64>,
}

/// Selection rules applied to the torrent's file set after metadata discovery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileSelectionRules {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub skip_fluff: bool,
}

/// Request payload for updating an existing torrent's file selection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileSelectionUpdate {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub skip_fluff: bool,
    #[serde(default)]
    pub priorities: Vec<FilePriorityOverride>,
}

/// Per-file priority override.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePriorityOverride {
    pub index: u32,
    pub priority: FilePriority,
}

/// Priority level recognized by libtorrent.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilePriority {
    Skip,
    Low,
    #[default]
    Normal,
    High,
}

/// Options controlling how the engine removes torrents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct RemoveTorrent {
    #[serde(default)]
    pub with_data: bool,
}

/// Lightweight transfer statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentRates {
    #[serde(default)]
    pub download_bps: u64,
    #[serde(default)]
    pub upload_bps: u64,
    #[serde(default)]
    pub ratio: f64,
}

/// Aggregated progress metrics for a torrent.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    #[serde(default)]
    pub eta_seconds: Option<u64>,
}

impl TorrentProgress {
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn percent_complete(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            (self.bytes_downloaded as f64 / self.bytes_total as f64) * 100.0
        }
    }
}

/// Individual file exposed by a torrent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    pub index: u32,
    pub path: String,
    pub size_bytes: u64,
    pub bytes_completed: u64,
    pub priority: FilePriority,
    pub selected: bool,
}

/// High-level torrent status surfaced by the inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentStatus {
    pub id: Uuid,
    pub name: Option<String>,
    pub state: revaer_events::TorrentState,
    pub progress: TorrentProgress,
    pub rates: TorrentRates,
    pub files: Option<Vec<TorrentFile>>,
    pub library_path: Option<String>,
    pub download_dir: Option<String>,
    pub sequential: bool,
    pub added_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
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
    FilesDiscovered {
        torrent_id: Uuid,
        files: Vec<TorrentFile>,
    },
    Progress {
        torrent_id: Uuid,
        progress: TorrentProgress,
        rates: TorrentRates,
    },
    StateChanged {
        torrent_id: Uuid,
        state: revaer_events::TorrentState,
    },
    Completed {
        torrent_id: Uuid,
        library_path: String,
    },
    MetadataUpdated {
        torrent_id: Uuid,
        name: Option<String>,
        download_dir: Option<String>,
    },
    Error {
        torrent_id: Uuid,
        message: String,
    },
}

/// Primary engine trait implemented by adapters (e.g. libtorrent).
#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported by this engine");
    }

    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported by this engine");
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported by this engine");
    }

    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported by this engine");
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported by this engine");
    }

    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported by this engine");
    }

    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported by this engine");
    }
}

/// Workflow façade exposed to the API layer for torrent lifecycle control.
#[async_trait]
pub trait TorrentWorkflow: Send + Sync {
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported");
    }

    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported");
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported");
    }

    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported");
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported");
    }

    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported");
    }

    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported");
    }
}

/// Inspector trait used by API consumers to fetch torrent snapshots.
#[async_trait]
pub trait TorrentInspector: Send + Sync {
    async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>>;

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>>;
}
