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

/// Runtime parameters applied to the libtorrent session.
#[derive(Debug, Clone)]
pub struct EngineRuntimeConfig {
    /// Root directory used for new torrent data.
    pub download_root: String,
    /// Directory where fast-resume payloads are stored.
    pub resume_dir: String,
    /// Whether the distributed hash table is enabled for peer discovery.
    pub enable_dht: bool,
    /// Whether torrents default to sequential download order.
    pub sequential_default: bool,
    /// Optional listen port override for the session.
    pub listen_port: Option<i32>,
    /// Optional limit for the number of active torrents.
    pub max_active: Option<i32>,
    /// Optional global download rate limit in bytes per second.
    pub download_rate_limit: Option<i64>,
    /// Optional global upload rate limit in bytes per second.
    pub upload_rate_limit: Option<i64>,
    /// Peer encryption requirements enforced by the engine.
    pub encryption: EncryptionPolicy,
}

/// Supported encryption policies exposed to the orchestration layer.
#[derive(Debug, Clone, Copy)]
pub enum EncryptionPolicy {
    /// Enforce encrypted peers exclusively.
    Require,
    /// Prefer encrypted peers but permit plaintext fallback.
    Prefer,
    /// Disable encrypted connections entirely.
    Disable,
}

impl EncryptionPolicy {
    #[must_use]
    /// Convert the policy to the numeric representation expected by libtorrent.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Require => 0,
            Self::Prefer => 1,
            Self::Disable => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EncryptionPolicy;

    #[test]
    fn encryption_policy_maps_to_expected_values() {
        assert_eq!(EncryptionPolicy::Require.as_u8(), 0);
        assert_eq!(EncryptionPolicy::Prefer.as_u8(), 1);
        assert_eq!(EncryptionPolicy::Disable.as_u8(), 2);
    }
}
