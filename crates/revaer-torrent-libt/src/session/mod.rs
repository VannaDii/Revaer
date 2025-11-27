#![allow(clippy::redundant_pub_crate)]

use crate::command::EngineRuntimeConfig;
use anyhow::Result;
use async_trait::async_trait;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit,
};
use uuid::Uuid;

#[cfg(feature = "libtorrent")]
mod native;
#[cfg(any(test, not(feature = "libtorrent")))]
mod stub;

#[cfg(test)]
pub(crate) use stub::StubSession;

#[async_trait]
pub(crate) trait LibtSession: Send {
    async fn add_torrent(&mut self, request: &AddTorrent) -> Result<()>;
    async fn remove_torrent(&mut self, id: Uuid, options: &RemoveTorrent) -> Result<()>;
    async fn pause_torrent(&mut self, id: Uuid) -> Result<()>;
    async fn resume_torrent(&mut self, id: Uuid) -> Result<()>;
    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> Result<()>;
    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> Result<()>;
    async fn update_limits(&mut self, id: Option<Uuid>, limits: &TorrentRateLimit) -> Result<()>;
    async fn update_selection(&mut self, id: Uuid, rules: &FileSelectionUpdate) -> Result<()>;
    async fn reannounce(&mut self, id: Uuid) -> Result<()>;
    async fn recheck(&mut self, id: Uuid) -> Result<()>;
    async fn poll_events(&mut self) -> Result<Vec<EngineEvent>>;
    async fn apply_config(&mut self, config: &EngineRuntimeConfig) -> Result<()>;
}

pub(crate) fn create_session() -> Result<Box<dyn LibtSession>> {
    #[cfg(feature = "libtorrent")]
    {
        native::create_session()
    }

    #[cfg(not(feature = "libtorrent"))]
    {
        Ok(Box::new(stub::StubSession::default()))
    }
}
