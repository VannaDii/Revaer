//! Shared DTOs matching the Phase 1 API and SSE payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
use web_sys::File;

/// Dashboard snapshot used by the UI and API client.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct DashboardSnapshot {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub active: u32,
    pub paused: u32,
    pub completed: u32,
    pub disk_total_gb: u32,
    pub disk_used_gb: u32,
    pub paths: Vec<PathUsage>,
    pub recent_events: Vec<DashboardEvent>,
    pub tracker_health: TrackerHealth,
    pub queue: QueueStatus,
    pub vpn: VpnState,
}

/// Disk usage per path for the dashboard view.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct PathUsage {
    pub label: &'static str,
    pub used_gb: u32,
    pub total_gb: u32,
}

/// Event entry displayed in the dashboard recent events list.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct DashboardEvent {
    pub label: &'static str,
    pub detail: &'static str,
    pub kind: EventKind,
}

/// Event severity kinds for dashboard events.
#[allow(dead_code, missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Info,
    Warning,
    Error,
}

/// Tracker health aggregates.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerHealth {
    pub ok: u16,
    pub warn: u16,
    pub error: u16,
}

/// Queue status aggregates.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct QueueStatus {
    pub active: u16,
    pub paused: u16,
    pub queued: u16,
    pub depth: u16,
}

/// VPN state summary for the dashboard.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct VpnState {
    pub state: String,
    pub message: String,
    pub last_change: String,
}

/// Toast variants used across the UI.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// Toast payload used by the host and app state.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub kind: ToastKind,
}

/// Navigation labels supplied by the router shell.
#[allow(missing_docs)]
#[derive(Clone, PartialEq)]
pub struct NavLabels {
    pub dashboard: String,
    pub torrents: String,
    pub search: String,
    pub jobs: String,
    pub settings: String,
    pub logs: String,
}

/// SSE connection state shared across shell/status components.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseState {
    Connected,
    Reconnecting {
        retry_in_secs: u8,
        last_event: &'static str,
        reason: &'static str,
    },
}

/// Dialog confirmation kinds for torrent actions.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmKind {
    Delete,
    DeleteData,
    Recheck,
}

/// Torrent add payload accepted by the API and UI.
#[cfg(target_arch = "wasm32")]
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub struct AddTorrentInput {
    pub value: Option<String>,
    pub file: Option<File>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub save_path: Option<String>,
}

#[cfg(target_arch = "wasm32")]
impl PartialEq for AddTorrentInput {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.category == other.category
            && self.tags == other.tags
            && self.save_path == other.save_path
            && self.file.as_ref().map(|f| f.name()) == other.file.as_ref().map(|f| f.name())
    }
}

/// Torrent detail view node representation.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct FileNode {
    pub name: String,
    pub size_gb: f32,
    pub completed_gb: f32,
    pub priority: String,
    pub wanted: bool,
    pub children: Vec<FileNode>,
}

/// Peer row for detail pane.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct PeerRow {
    pub ip: String,
    pub client: String,
    pub flags: String,
    pub country: String,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub progress: f32,
}

/// Tracker row for detail pane.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerRow {
    pub url: String,
    pub status: String,
    pub next_announce: String,
    pub last_error: Option<String>,
}

/// Event row for detail pane.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct EventRow {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Metadata section for detail pane.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub hash: String,
    pub magnet: String,
    pub size_gb: f32,
    pub piece_count: u32,
    pub piece_size_mb: u32,
}

/// Aggregated detail data used by the UI.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub struct DetailData {
    pub name: String,
    pub files: Vec<FileNode>,
    pub peers: Vec<PeerRow>,
    pub trackers: Vec<TrackerRow>,
    pub events: Vec<EventRow>,
    pub metadata: Metadata,
}

/// Demo snapshot used by the initial UI shell.
#[must_use]
pub fn demo_snapshot() -> DashboardSnapshot {
    DashboardSnapshot {
        download_bps: 142_000_000,
        upload_bps: 22_000_000,
        active: 12,
        paused: 4,
        completed: 187,
        disk_total_gb: 4200,
        disk_used_gb: 2830,
        paths: vec![
            PathUsage {
                label: "/data/media",
                used_gb: 1800,
                total_gb: 2600,
            },
            PathUsage {
                label: "/data/incomplete",
                used_gb: 120,
                total_gb: 400,
            },
            PathUsage {
                label: "/data/archive",
                used_gb: 910,
                total_gb: 1200,
            },
        ],
        recent_events: vec![
            DashboardEvent {
                label: "Tracker warn",
                detail: "udp://tracker.example: announce timeout; retrying in 5m",
                kind: EventKind::Warning,
            },
            DashboardEvent {
                label: "Filesystem move",
                detail: "Moved The.Expanse.S01E05 → /media/tv/The Expanse/Season 1",
                kind: EventKind::Info,
            },
            DashboardEvent {
                label: "Tracker failure",
                detail: "http://tracker.down: failed with 502 after retries",
                kind: EventKind::Error,
            },
            DashboardEvent {
                label: "VPN reconnection",
                detail: "Recovered tunnel after 12s; session resumed",
                kind: EventKind::Info,
            },
        ],
        tracker_health: TrackerHealth {
            ok: 24,
            warn: 3,
            error: 1,
        },
        queue: QueueStatus {
            active: 12,
            paused: 4,
            queued: 18,
            depth: 34,
        },
        vpn: VpnState {
            state: "connected".into(),
            message: "Routing through wg0".into(),
            last_change: "12s ago".into(),
        },
    }
}

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

impl From<TorrentDetail> for DetailData {
    fn from(detail: TorrentDetail) -> Self {
        let files = detail
            .files
            .into_iter()
            .map(|file| FileNode {
                name: file.path,
                size_gb: file.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                completed_gb: file.completed_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                priority: file.priority,
                wanted: file.wanted,
                children: vec![],
            })
            .collect();
        let peers = detail
            .peers
            .into_iter()
            .map(|peer| PeerRow {
                ip: peer.ip,
                client: peer.client,
                flags: peer.flags,
                country: peer.country.unwrap_or_default(),
                download_bps: peer.download_bps,
                upload_bps: peer.upload_bps,
                progress: peer.progress,
            })
            .collect();
        let trackers = detail
            .trackers
            .into_iter()
            .map(|tracker| TrackerRow {
                url: tracker.url,
                status: tracker.status,
                next_announce: tracker.next_announce_at.unwrap_or_else(|| "-".to_string()),
                last_error: tracker.last_error,
            })
            .collect();
        let events = detail
            .events
            .into_iter()
            .map(|event| EventRow {
                timestamp: event.timestamp,
                level: event.level,
                message: event.message,
            })
            .collect();
        Self {
            name: detail.name,
            files,
            peers,
            trackers,
            events,
            metadata: Metadata {
                hash: detail.hash,
                magnet: detail.magnet,
                size_gb: detail.size_bytes as f32 / (1024.0 * 1024.0 * 1024.0),
                piece_count: detail.piece_count,
                piece_size_mb: detail.piece_size_bytes / 1024 / 1024,
            },
        }
    }
}

/// Demo detail record used by the torrent view.
#[must_use]
pub fn demo_detail(id: &str) -> Option<DetailData> {
    let name = match id {
        "2" => "The.Expanse.S01E05.1080p.BluRay.DTS.x264",
        "3" => "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1",
        "4" => "Ubuntu-24.04.1-live-server-amd64.iso",
        "5" => "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264",
        _ => "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10",
    };

    Some(DetailData {
        name: name.to_string(),
        files: vec![
            FileNode {
                name: "Foundation.S02E08.mkv".to_string(),
                size_gb: 14.2,
                completed_gb: 6.1,
                priority: "high".to_string(),
                wanted: true,
                children: vec![],
            },
            FileNode {
                name: "Extras".to_string(),
                size_gb: 3.2,
                completed_gb: 1.4,
                priority: "normal".to_string(),
                wanted: true,
                children: vec![
                    FileNode {
                        name: "Featurette-01.mkv".to_string(),
                        size_gb: 1.1,
                        completed_gb: 1.1,
                        priority: "normal".to_string(),
                        wanted: true,
                        children: vec![],
                    },
                    FileNode {
                        name: "Interview-01.mkv".to_string(),
                        size_gb: 0.9,
                        completed_gb: 0.2,
                        priority: "low".to_string(),
                        wanted: false,
                        children: vec![],
                    },
                ],
            },
        ],
        peers: vec![
            PeerRow {
                ip: "203.0.113.24".to_string(),
                client: "qBittorrent 4.6".to_string(),
                flags: "DIXE".to_string(),
                country: "CA".to_string(),
                download_bps: 8_400_000,
                upload_bps: 650_000,
                progress: 0.54,
            },
            PeerRow {
                ip: "198.51.100.18".to_string(),
                client: "Transmission 4.0".to_string(),
                flags: "UXE".to_string(),
                country: "US".to_string(),
                download_bps: 2_200_000,
                upload_bps: 90_000,
                progress: 0.31,
            },
        ],
        trackers: vec![
            TrackerRow {
                url: "udp://tracker.example:6969".to_string(),
                status: "online".to_string(),
                next_announce: "in 3m".to_string(),
                last_error: None,
            },
            TrackerRow {
                url: "http://tracker.down/announce".to_string(),
                status: "error".to_string(),
                next_announce: "retrying".to_string(),
                last_error: Some("502 Bad Gateway".to_string()),
            },
        ],
        events: vec![
            EventRow {
                timestamp: "12:04:11".to_string(),
                level: "info".to_string(),
                message: "Resumed torrent after pause".to_string(),
            },
            EventRow {
                timestamp: "12:03:55".to_string(),
                level: "warn".to_string(),
                message: "Tracker timed out; will retry".to_string(),
            },
        ],
        metadata: Metadata {
            hash: "123456ABCDEF".into(),
            magnet: "magnet:?xt=urn:btih:123456ABCDEF".into(),
            size_gb: 14.2,
            piece_count: 1120,
            piece_size_mb: 8,
        },
    })
}
