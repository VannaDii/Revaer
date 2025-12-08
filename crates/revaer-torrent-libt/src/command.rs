use crate::types::EngineRuntimeConfig;
use revaer_torrent_core::{AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit};
use uuid::Uuid;

/// Command definitions and runtime configuration inputs for the libtorrent worker.

#[derive(Debug)]
pub enum EngineCommand {
    /// Add a torrent to the session.
    Add(AddTorrent),
    /// Remove a torrent from the session, optionally deleting its data.
    Remove {
        /// Unique torrent identifier.
        id: Uuid,
        /// Removal behaviour and cleanup options.
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
    /// Override sequential download behaviour for a torrent.
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
    /// Force tracker reannounce for a torrent.
    Reannounce {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Recheck torrent data integrity.
    Recheck {
        /// Unique torrent identifier.
        id: Uuid,
    },
    /// Apply a new runtime configuration profile.
    ApplyConfig(EngineRuntimeConfig),
}
