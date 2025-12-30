use crate::types::{EngineRuntimeConfig, EngineSettingsSnapshot};
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit, TorrentResult,
    model::{
        TorrentAuthorRequest, TorrentAuthorResult, TorrentOptionsUpdate, TorrentTrackersUpdate,
        TorrentWebSeedsUpdate,
    },
};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Command definitions and runtime configuration inputs for the libtorrent worker.

#[derive(Debug)]
pub enum EngineCommand {
    /// Add a torrent to the session.
    Add(Box<AddTorrent>),
    /// Author a new `.torrent` metainfo payload.
    CreateTorrent {
        /// Authoring request parameters.
        request: TorrentAuthorRequest,
        /// Channel used to return the authoring result.
        respond_to: oneshot::Sender<TorrentResult<TorrentAuthorResult>>,
    },
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
        respond_to: oneshot::Sender<TorrentResult<Vec<PeerSnapshot>>>,
    },
    /// Set or clear a streaming deadline for a piece.
    SetPieceDeadline {
        /// Unique torrent identifier.
        id: Uuid,
        /// Piece index to target.
        piece: u32,
        /// Deadline in milliseconds; when absent the deadline is cleared.
        deadline_ms: Option<u32>,
    },
    /// Inspect applied native session settings for integration tests.
    InspectSettings {
        /// Channel used to return the settings snapshot.
        respond_to: oneshot::Sender<TorrentResult<EngineSettingsSnapshot>>,
    },
}

impl EngineCommand {
    pub(crate) const fn operation(&self) -> &'static str {
        match self {
            Self::Add(_) => "add_torrent",
            Self::CreateTorrent { .. } => "create_torrent",
            Self::Remove { .. } => "remove_torrent",
            Self::Pause { .. } => "pause_torrent",
            Self::Resume { .. } => "resume_torrent",
            Self::SetSequential { .. } => "set_sequential",
            Self::UpdateLimits { .. } => "update_limits",
            Self::UpdateSelection { .. } => "update_selection",
            Self::UpdateOptions { .. } => "update_options",
            Self::UpdateTrackers { .. } => "update_trackers",
            Self::UpdateWebSeeds { .. } => "update_web_seeds",
            Self::Reannounce { .. } => "reannounce",
            Self::MoveStorage { .. } => "move_torrent",
            Self::Recheck { .. } => "recheck",
            Self::ApplyConfig(_) => "apply_config",
            Self::QueryPeers { .. } => "query_peers",
            Self::SetPieceDeadline { .. } => "set_piece_deadline",
            Self::InspectSettings { .. } => "inspect_settings",
        }
    }

    pub(crate) fn torrent_id(&self) -> Option<Uuid> {
        match self {
            Self::Add(request) => Some(request.id),
            Self::Remove { id, .. }
            | Self::Pause { id }
            | Self::Resume { id }
            | Self::SetSequential { id, .. }
            | Self::UpdateSelection { id, .. }
            | Self::UpdateOptions { id, .. }
            | Self::UpdateTrackers { id, .. }
            | Self::UpdateWebSeeds { id, .. }
            | Self::Reannounce { id }
            | Self::MoveStorage { id, .. }
            | Self::Recheck { id }
            | Self::QueryPeers { id, .. }
            | Self::SetPieceDeadline { id, .. } => Some(*id),
            Self::UpdateLimits { id, .. } => *id,
            Self::CreateTorrent { .. } | Self::ApplyConfig(_) | Self::InspectSettings { .. } => {
                None
            }
        }
    }
}
