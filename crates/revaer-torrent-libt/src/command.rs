use crate::types::EngineRuntimeConfig;
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
    model::{TorrentOptionsUpdate, TorrentTrackersUpdate, TorrentWebSeedsUpdate},
};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Command definitions and runtime configuration inputs for the libtorrent worker.

#[derive(Debug)]
pub enum EngineCommand {
    /// Add a torrent to the session.
    Add(Box<AddTorrent>),
    /// Remove a torrent from the session, optionally deleting its data.
    Remove {
        /// Unique torrent identifier.
        id: Uuid,
        /// Removal behavior and cleanup options.
        options: RemoveTorrent,
    },
    /// Pause an active torrent without removing it.
    Pause {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Resume a paused torrent.
    Resume {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Override sequential download behavior for a torrent.
    SetSequential {
        /// Unique torrent identifier.
        id: Uuid,
        /// Whether sequential download should be enabled.
        sequential: bool,
    },
    /// Apply rate limits globally or for a specific torrent.
    UpdateLimits {
        /// Target torrent; `None` applies the limits globally.
        id: Option<Uuid>,
        /// Rate limit configuration.
        limits: TorrentRateLimit,
    },
    /// Update file selection rules for a torrent.
    UpdateSelection {
        /// Unique torrent identifier.
        id: Uuid,
        /// Inclusion, exclusion, and priority rules to apply.
        rules: FileSelectionUpdate,
    },
    /// Update per-torrent options after admission.
    UpdateOptions {
        /// Unique torrent identifier.
        id: Uuid,
        /// New options to apply.
        options: TorrentOptionsUpdate,
    },
    /// Update trackers for a torrent.
    UpdateTrackers {
        /// Unique torrent identifier.
        id: Uuid,
        /// Trackers to apply.
        trackers: TorrentTrackersUpdate,
    },
    /// Update web seeds for a torrent.
    UpdateWebSeeds {
        /// Unique torrent identifier.
        id: Uuid,
        /// Web seeds to apply.
        web_seeds: TorrentWebSeedsUpdate,
    },
    /// Force tracker reannounce for a torrent.
    Reannounce {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Move torrent storage to a new directory.
    MoveStorage {
        /// Unique torrent identifier.
        id: Uuid,
        /// Destination download directory.
        download_dir: String,
    },
    /// Recheck torrent data integrity.
    Recheck {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Apply a new runtime configuration profile.
    ApplyConfig(Box<EngineRuntimeConfig>),
    /// Inspect peers connected to a torrent.
    QueryPeers {
        /// Unique torrent identifier.
        id: Uuid,
        /// Channel used to return peer snapshots.
        respond_to: oneshot::Sender<anyhow::Result<Vec<PeerSnapshot>>>,
    },
}
