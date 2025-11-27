//! Shared DTOs matching the Phase 1 API and SSE payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Minimal torrent summary returned from the Phase 1 API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorrentSummary {
    /// Stable torrent identifier.
    pub id: Uuid,
    /// Display name for the torrent payload.
    pub name: String,
    /// Current status string from the engine.
    pub status: String,
    /// Completion percentage in the range 0.0–1.0.
    pub progress: f32,
    /// Optional ETA in seconds; `None` when unknown.
    pub eta_seconds: Option<u64>,
    /// Upload ratio as reported by the engine.
    pub ratio: f32,
    /// Arbitrary labels applied to the torrent.
    pub tags: Vec<String>,
    /// Tracker URL, if present.
    pub tracker: Option<String>,
    /// Save path for the torrent contents.
    pub save_path: Option<String>,
    /// Category assigned by the user or client.
    pub category: Option<String>,
    /// Total payload size in bytes.
    pub size_bytes: u64,
    /// Current download speed in bytes per second.
    pub download_bps: u64,
    /// Current upload speed in bytes per second.
    pub upload_bps: u64,
    /// Optional RFC3339 timestamp when the torrent was added.
    pub added_at: Option<String>,
    /// Optional RFC3339 timestamp when the torrent completed.
    pub completed_at: Option<String>,
}

/// File entry within a torrent payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorrentFile {
    /// File path relative to the torrent root.
    pub path: String,
    /// Total file size in bytes.
    pub size_bytes: u64,
    /// Completed bytes downloaded.
    pub completed_bytes: u64,
    /// Engine priority label for the file.
    pub priority: String,
    /// Whether the file is wanted for download.
    pub wanted: bool,
}

/// Peer information for swarm diagnostics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    /// Peer IP address.
    pub ip: String,
    /// Client string (e.g., qBittorrent).
    pub client: String,
    /// Raw flag string from the tracker.
    pub flags: String,
    /// Optional country code for geo display.
    pub country: Option<String>,
    /// Download speed from this peer in bytes per second.
    pub download_bps: u64,
    /// Upload speed to this peer in bytes per second.
    pub upload_bps: u64,
    /// Completion percentage for the peer payload.
    pub progress: f32,
}

/// Tracker status record used in the UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tracker {
    /// Tracker announce URL.
    pub url: String,
    /// Current tracker status string.
    pub status: String,
    /// Next announce time in RFC3339, when available.
    pub next_announce_at: Option<String>,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Timestamp for last error, if reported.
    pub last_error_at: Option<String>,
}

/// Server-Sent Event payloads emitted by the backend.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum SseEvent {
    /// Progress update for a torrent.
    #[serde(rename = "torrent_progress")]
    TorrentProgress {
        /// Target torrent identifier.
        torrent_id: Uuid,
        /// New progress value (0.0–1.0).
        progress: f32,
        /// Optional ETA in seconds.
        eta_seconds: Option<u64>,
        /// Download rate in bytes per second.
        download_bps: u64,
        /// Upload rate in bytes per second.
        upload_bps: u64,
    },
    /// State change for a torrent.
    #[serde(rename = "torrent_state")]
    TorrentState {
        /// Target torrent identifier.
        torrent_id: Uuid,
        /// New torrent status string.
        status: String,
        /// Optional reason for the status change.
        reason: Option<String>,
    },
    /// Rate update for a torrent.
    #[serde(rename = "torrent_rates")]
    TorrentRates {
        /// Target torrent identifier.
        torrent_id: Uuid,
        /// Download rate in bytes per second.
        download_bps: u64,
        /// Upload rate in bytes per second.
        upload_bps: u64,
    },
    /// Notification that a torrent was added.
    #[serde(rename = "torrent_added")]
    TorrentAdded {
        /// Newly added torrent identifier.
        torrent_id: Uuid,
    },
    /// Notification that a torrent was removed.
    #[serde(rename = "torrent_removed")]
    TorrentRemoved {
        /// Removed torrent identifier.
        torrent_id: Uuid,
    },
    /// Job queue update.
    #[serde(rename = "jobs_update")]
    JobsUpdate {
        /// Current job queue snapshot.
        jobs: Vec<Job>,
    },
    /// VPN state update.
    #[serde(rename = "vpn_state")]
    VpnState {
        /// VPN state string.
        state: String,
        /// VPN status message suitable for UI display.
        message: String,
        /// Last change timestamp in RFC3339.
        last_change: String,
    },
    /// Aggregate system throughput.
    #[serde(rename = "system_rates")]
    SystemRates {
        /// Aggregate download throughput in bytes per second.
        download_bps: u64,
        /// Aggregate upload throughput in bytes per second.
        upload_bps: u64,
    },
    /// Queue status snapshot.
    #[serde(rename = "queue_status")]
    QueueStatus {
        /// Active torrents count.
        active: u32,
        /// Paused torrents count.
        paused: u32,
        /// Queued torrents count.
        queued: u32,
        /// Queue depth (pending work).
        depth: u32,
    },
}

/// Background job metadata for the queue view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    /// Job identifier.
    pub id: Uuid,
    /// Optional torrent identifier the job relates to.
    pub torrent_id: Option<Uuid>,
    /// Job kind (e.g., hash check).
    pub kind: String,
    /// Current job status string.
    pub status: String,
    /// Optional details or failure context.
    pub detail: Option<String>,
    /// Last update timestamp in RFC3339.
    pub updated_at: String,
}

/// Event log entry for torrent detail views.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetailEvent {
    /// Timestamp string (RFC3339 or time-of-day).
    pub timestamp: String,
    /// Severity level (info/warn/error).
    pub level: String,
    /// Human-readable message.
    pub message: String,
}

/// Detailed torrent view including files, peers, trackers, and metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorrentDetail {
    /// Torrent identifier.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// File tree for the torrent.
    pub files: Vec<TorrentFile>,
    /// Peers connected to the torrent.
    pub peers: Vec<Peer>,
    /// Tracker list with announce metadata.
    pub trackers: Vec<Tracker>,
    /// Event log for the torrent lifecycle.
    pub events: Vec<DetailEvent>,
    /// Info hash for the torrent.
    pub hash: String,
    /// Magnet URI representation.
    pub magnet: String,
    /// Total payload size in bytes.
    pub size_bytes: u64,
    /// Total piece count.
    pub piece_count: u32,
    /// Piece size in bytes.
    pub piece_size_bytes: u32,
}
