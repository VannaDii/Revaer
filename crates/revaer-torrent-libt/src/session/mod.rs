use crate::types::{EngineRuntimeConfig, EngineSettingsSnapshot};
use async_trait::async_trait;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
    TorrentResult,
};
use uuid::Uuid;

#[cfg(libtorrent_native)]
mod options;

/// Native libtorrent session implementation backed by C++ bindings.
#[cfg(libtorrent_native)]
pub mod native;
/// Stub session implementation used for non-native targets/tests.
#[cfg(any(test, not(libtorrent_native)))]
pub mod stub;

/// Session abstraction for the libtorrent bridge, with native and stub backends.
#[cfg(test)]
pub(crate) use stub::StubSession;

/// Abstraction over the native libtorrent session surface.
#[async_trait]
pub trait LibTorrentSession: Send {
    /// Add a new torrent to the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the native bridge rejects the request.
    async fn add_torrent(&mut self, request: &AddTorrent) -> TorrentResult<()>;
    /// Author a new `.torrent` metainfo payload.
    ///
    /// # Errors
    ///
    /// Returns an error if authoring fails.
    async fn create_torrent(
        &mut self,
        request: &revaer_torrent_core::model::TorrentAuthorRequest,
    ) -> TorrentResult<revaer_torrent_core::model::TorrentAuthorResult>;
    /// Remove a torrent from the session.
    ///
    /// # Errors
    ///
    /// Returns an error if the torrent cannot be removed.
    async fn remove_torrent(&mut self, id: Uuid, options: &RemoveTorrent) -> TorrentResult<()>;
    /// Pause an active torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the session fails to process the pause request.
    async fn pause_torrent(&mut self, id: Uuid) -> TorrentResult<()>;
    /// Resume a paused torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the session fails to resume the torrent.
    async fn resume_torrent(&mut self, id: Uuid) -> TorrentResult<()>;
    /// Toggle sequential download behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if the sequential preference cannot be persisted.
    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> TorrentResult<()>;
    /// Load fast-resume payload for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be applied.
    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> TorrentResult<()>;
    /// Apply rate limits globally or to a specific torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the limits cannot be persisted.
    async fn update_limits(
        &mut self,
        id: Option<Uuid>,
        limits: &TorrentRateLimit,
    ) -> TorrentResult<()>;
    /// Update file selection rules for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the rules cannot be applied.
    async fn update_selection(
        &mut self,
        id: Uuid,
        rules: &FileSelectionUpdate,
    ) -> TorrentResult<()>;
    /// Update per-torrent options after admission.
    ///
    /// # Errors
    ///
    /// Returns an error if the options cannot be applied.
    async fn update_options(
        &mut self,
        id: Uuid,
        options: &revaer_torrent_core::model::TorrentOptionsUpdate,
    ) -> TorrentResult<()>;
    /// Replace or append trackers for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if tracker updates cannot be applied.
    async fn update_trackers(
        &mut self,
        id: Uuid,
        trackers: &revaer_torrent_core::model::TorrentTrackersUpdate,
    ) -> TorrentResult<()>;
    /// Replace or append web seeds for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if web seed updates cannot be applied.
    async fn update_web_seeds(
        &mut self,
        id: Uuid,
        web_seeds: &revaer_torrent_core::model::TorrentWebSeedsUpdate,
    ) -> TorrentResult<()>;
    /// Set or clear a deadline for a specific piece.
    ///
    /// # Errors
    ///
    /// Returns an error if the deadline cannot be applied.
    async fn set_piece_deadline(
        &mut self,
        id: Uuid,
        piece: u32,
        deadline_ms: Option<u32>,
    ) -> TorrentResult<()>;
    /// Trigger tracker reannounce for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be queued.
    async fn reannounce(&mut self, id: Uuid) -> TorrentResult<()>;
    /// Move torrent storage to a new download directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the move cannot be scheduled.
    async fn move_torrent(&mut self, id: Uuid, download_dir: &str) -> TorrentResult<()>;
    /// Verify on-disk data for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the recheck cannot be scheduled.
    async fn recheck(&mut self, id: Uuid) -> TorrentResult<()>;
    /// Inspect connected peers for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if peer metadata cannot be fetched.
    async fn peers(&mut self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>>;
    /// Drain pending events from the session.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching the events fails.
    async fn poll_events(&mut self) -> TorrentResult<Vec<EngineEvent>>;
    /// Apply a runtime configuration profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be applied.
    async fn apply_config(&mut self, config: &EngineRuntimeConfig) -> TorrentResult<()>;
    /// Inspect applied native session settings.
    ///
    /// # Errors
    ///
    /// Returns an error if settings cannot be retrieved.
    async fn inspect_settings(&mut self) -> TorrentResult<EngineSettingsSnapshot>;
}

/// Construct a libtorrent session using the native bindings when available.
///
/// # Errors
///
/// Returns an error if the native session cannot be initialized.
pub fn create_session() -> TorrentResult<Box<dyn LibTorrentSession>> {
    #[cfg(libtorrent_native)]
    {
        native::create_session()
    }

    #[cfg(not(libtorrent_native))]
    {
        Ok(Box::new(stub::StubSession::default()))
    }
}
