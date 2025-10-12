//! Libtorrent adapter stub.

use anyhow::Result;
use revaer_torrent_core::{TorrentDescriptor, TorrentEngine};
use tracing::info;
use uuid::Uuid;

pub struct LibtorrentEngine;

#[async_trait::async_trait]
impl TorrentEngine for LibtorrentEngine {
    async fn add_torrent(&self, descriptor: TorrentDescriptor) -> Result<()> {
        info!("Pretend to add torrent {}", descriptor.name);
        Ok(())
    }

    async fn remove_torrent(&self, id: Uuid) -> Result<()> {
        info!("Pretend to remove torrent {}", id);
        Ok(())
    }
}
