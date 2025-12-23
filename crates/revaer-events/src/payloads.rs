//! Event payload types carried across the platform.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Identifier assigned to each event emitted by the platform.
pub type EventId = u64;

/// Default buffer size for the in-memory replay ring.
pub const DEFAULT_REPLAY_CAPACITY: usize = 1_024;

/// Typed domain events surfaced across the system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// A torrent was registered with the engine.
    TorrentAdded {
        /// Identifier for the torrent that was added.
        torrent_id: Uuid,
        /// Display name associated with the torrent metadata.
        name: String,
    },
    /// Torrent metadata finished scanning and produced a file listing.
    FilesDiscovered {
        /// Identifier for the torrent that produced the listing.
        torrent_id: Uuid,
        /// Collection of files discovered within the torrent payload.
        files: Vec<DiscoveredFile>,
    },
    /// Periodic progress update emitted while a torrent is downloading.
    Progress {
        /// Identifier for the torrent being tracked.
        torrent_id: Uuid,
        /// Aggregate bytes downloaded so far.
        bytes_downloaded: u64,
        /// Total bytes expected for the torrent payload.
        bytes_total: u64,
    },
    /// Torrent transitioned into a new lifecycle state.
    StateChanged {
        /// Identifier for the torrent whose state changed.
        torrent_id: Uuid,
        /// Updated state snapshot.
        state: TorrentState,
    },
    /// Torrent finished processing and the library artifact is ready.
    Completed {
        /// Identifier for the completed torrent.
        torrent_id: Uuid,
        /// Absolute path to the final library artifact.
        library_path: String,
    },
    /// Torrent metadata changed (name or download directory).
    MetadataUpdated {
        /// Identifier for the torrent whose metadata changed.
        torrent_id: Uuid,
        /// Optional updated display name.
        name: Option<String>,
        /// Optional updated download directory.
        download_dir: Option<String>,
        /// Optional updated comment.
        comment: Option<String>,
        /// Optional updated source label.
        source: Option<String>,
        /// Optional updated private flag.
        private: Option<bool>,
    },
    /// Torrent was removed from the catalog.
    TorrentRemoved {
        /// Identifier for the torrent that was removed.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing pipeline started for a torrent.
    FsopsStarted {
        /// Identifier for the torrent undergoing filesystem processing.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing reported an intermediate step completion.
    FsopsProgress {
        /// Identifier for the torrent undergoing filesystem processing.
        torrent_id: Uuid,
        /// Name of the pipeline step that completed.
        step: String,
    },
    /// Filesystem post-processing completed successfully.
    FsopsCompleted {
        /// Identifier for the torrent whose filesystem processing completed.
        torrent_id: Uuid,
    },
    /// Filesystem post-processing failed with an error message.
    FsopsFailed {
        /// Identifier for the torrent whose filesystem processing failed.
        torrent_id: Uuid,
        /// Human-readable error detail describing the failure.
        message: String,
    },
    /// Configuration update was applied.
    SettingsChanged {
        /// Description of the applied configuration change.
        description: String,
    },
    /// System health status changed (degraded or restored components).
    HealthChanged {
        /// Components currently considered degraded.
        degraded: Vec<String>,
    },
    /// Torrent file selection was reconciled with the configured policy.
    SelectionReconciled {
        /// Identifier for the torrent whose selection changed.
        torrent_id: Uuid,
        /// Explanation for why the selection changed.
        reason: String,
    },
}

impl Event {
    /// Machine-friendly discriminator for SSE consumers.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TorrentAdded { .. } => "torrent_added",
            Self::FilesDiscovered { .. } => "files_discovered",
            Self::Progress { .. } => "progress",
            Self::StateChanged { .. } => "state_changed",
            Self::Completed { .. } => "completed",
            Self::MetadataUpdated { .. } => "metadata_updated",
            Self::TorrentRemoved { .. } => "torrent_removed",
            Self::FsopsStarted { .. } => "fsops_started",
            Self::FsopsProgress { .. } => "fsops_progress",
            Self::FsopsCompleted { .. } => "fsops_completed",
            Self::FsopsFailed { .. } => "fsops_failed",
            Self::SettingsChanged { .. } => "settings_changed",
            Self::HealthChanged { .. } => "health_changed",
            Self::SelectionReconciled { .. } => "selection_reconciled",
        }
    }
}

/// Metadata wrapper around events. Each envelope tracks the event id and emission timestamp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EventEnvelope {
    /// Monotonic identifier assigned to the wrapped event.
    pub id: EventId,
    /// Timestamp recording when the envelope was produced.
    pub timestamp: DateTime<Utc>,
    /// Wrapped event payload.
    pub event: Event,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_maps_torrent_variants() {
        assert_event_kind(
            &Event::TorrentAdded {
                torrent_id: Uuid::nil(),
                name: "demo".into(),
            },
            "torrent_added",
        );
        assert_event_kind(
            &Event::FilesDiscovered {
                torrent_id: Uuid::nil(),
                files: vec![],
            },
            "files_discovered",
        );
        assert_event_kind(
            &Event::Progress {
                torrent_id: Uuid::nil(),
                bytes_downloaded: 1,
                bytes_total: 2,
            },
            "progress",
        );
        assert_event_kind(
            &Event::StateChanged {
                torrent_id: Uuid::nil(),
                state: TorrentState::Queued,
            },
            "state_changed",
        );
        assert_event_kind(
            &Event::Completed {
                torrent_id: Uuid::nil(),
                library_path: "/tmp".into(),
            },
            "completed",
        );
        assert_event_kind(
            &Event::MetadataUpdated {
                torrent_id: Uuid::nil(),
                name: Some("demo".into()),
                download_dir: Some("/downloads/demo".into()),
                comment: None,
                source: None,
                private: None,
            },
            "metadata_updated",
        );
        assert_event_kind(
            &Event::TorrentRemoved {
                torrent_id: Uuid::nil(),
            },
            "torrent_removed",
        );
    }

    #[test]
    fn event_kind_maps_system_variants() {
        assert_event_kind(
            &Event::FsopsStarted {
                torrent_id: Uuid::nil(),
            },
            "fsops_started",
        );
        assert_event_kind(
            &Event::FsopsProgress {
                torrent_id: Uuid::nil(),
                step: "extract".into(),
            },
            "fsops_progress",
        );
        assert_event_kind(
            &Event::FsopsCompleted {
                torrent_id: Uuid::nil(),
            },
            "fsops_completed",
        );
        assert_event_kind(
            &Event::FsopsFailed {
                torrent_id: Uuid::nil(),
                message: "err".into(),
            },
            "fsops_failed",
        );
        assert_event_kind(
            &Event::SettingsChanged {
                description: "desc".into(),
            },
            "settings_changed",
        );
        assert_event_kind(
            &Event::HealthChanged {
                degraded: vec!["x".into()],
            },
            "health_changed",
        );
        assert_event_kind(
            &Event::SelectionReconciled {
                torrent_id: Uuid::nil(),
                reason: "policy".into(),
            },
            "selection_reconciled",
        );
    }

    #[test]
    fn envelope_carries_fields() {
        let event = Event::SettingsChanged {
            description: "desc".into(),
        };
        let envelope = EventEnvelope {
            id: 42,
            timestamp: Utc::now(),
            event: event.clone(),
        };
        assert_eq!(envelope.id, 42);
        assert_eq!(envelope.event, event);
    }

    fn assert_event_kind(event: &Event, expected: &str) {
        assert_eq!(event.kind(), expected);
    }
}

/// Individual file discovered within a torrent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DiscoveredFile {
    /// Relative path to the file inside the torrent contents.
    pub path: String,
    /// Size of the file in bytes.
    pub size_bytes: u64,
}

/// High-level torrent states that downstream consumers care about.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentState {
    /// Torrent has been queued but not yet started.
    Queued,
    /// Torrent is fetching metadata (e.g., magnet resolution).
    FetchingMetadata,
    /// Torrent is actively downloading payload data.
    Downloading,
    /// Torrent is seeding and uploading data to peers.
    Seeding,
    /// Torrent completed downloading and awaits post-processing.
    Completed,
    /// Torrent encountered an unrecoverable error with a description.
    Failed {
        /// Error detail describing why the torrent failed.
        message: String,
    },
    /// Torrent has been stopped manually and is inactive.
    Stopped,
}
