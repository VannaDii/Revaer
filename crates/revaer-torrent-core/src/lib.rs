//! Engine-agnostic torrent interfaces and DTOs.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use revaer_events::{DiscoveredFile, TorrentState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentDescriptor {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TorrentProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentStatus {
    pub id: Uuid,
    pub name: Option<String>,
    pub state: TorrentState,
    pub progress: TorrentProgress,
    pub files: Option<Vec<DiscoveredFile>>,
    pub library_path: Option<String>,
    pub last_updated: DateTime<Utc>,
}

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn add_torrent(&self, descriptor: TorrentDescriptor) -> anyhow::Result<()>;
    async fn remove_torrent(&self, id: Uuid) -> anyhow::Result<()>;
}

#[async_trait]
pub trait TorrentWorkflow: Send + Sync {
    async fn add_torrent(&self, descriptor: TorrentDescriptor) -> anyhow::Result<()>;
    async fn remove_torrent(&self, id: Uuid) -> anyhow::Result<()>;
}

#[async_trait]
pub trait TorrentInspector: Send + Sync {
    async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>>;
}
