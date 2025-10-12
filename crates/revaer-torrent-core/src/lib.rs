//! Engine-agnostic torrent interfaces and DTOs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentDescriptor {
    pub id: Uuid,
    pub name: String,
}

#[async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn add_torrent(&self, descriptor: TorrentDescriptor) -> anyhow::Result<()>;
    async fn remove_torrent(&self, id: Uuid) -> anyhow::Result<()>;
}
