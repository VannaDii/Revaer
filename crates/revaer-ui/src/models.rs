//! Shared DTOs matching the Phase 1 API and SSE payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
use web_sys::File;

const BYTES_PER_GB: f64 = 1024.0 * 1024.0 * 1024.0;
const TWO_POW_32: f64 = 4_294_967_296.0;

fn bytes_to_gb(bytes: u64) -> f64 {
    let high = u32::try_from(bytes >> 32).unwrap_or(0);
    let low = u32::try_from(bytes & 0xFFFF_FFFF).unwrap_or(0);
    ((f64::from(high) * TWO_POW_32) + f64::from(low)) / BYTES_PER_GB
}

/// Dashboard snapshot used by the UI and API client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardSnapshot {
    /// Aggregate download throughput in bytes per second.
    pub download_bps: u64,
    /// Aggregate upload throughput in bytes per second.
    pub upload_bps: u64,
    /// Count of active torrents.
    pub active: u32,
    /// Count of paused torrents.
    pub paused: u32,
    /// Count of completed torrents.
    pub completed: u32,
    /// Total disk capacity (GB).
    pub disk_total_gb: u32,
    /// Used disk capacity (GB).
    pub disk_used_gb: u32,
    /// Disk usage breakdown per path.
    pub paths: Vec<PathUsage>,
    /// Recent dashboard event entries.
    pub recent_events: Vec<DashboardEvent>,
    /// Tracker health summary.
    pub tracker_health: TrackerHealth,
    /// Queue status snapshot.
    pub queue: QueueStatus,
    /// VPN state summary.
    pub vpn: VpnState,
}

/// Disk usage per path for the dashboard view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathUsage {
    /// Display label for the path (e.g., mount point).
    pub label: &'static str,
    /// Used space in GB.
    pub used_gb: u32,
    /// Total space in GB.
    pub total_gb: u32,
}

/// Event entry displayed in the dashboard recent events list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardEvent {
    /// Short label for the event.
    pub label: &'static str,
    /// Secondary detail text for the event.
    pub detail: &'static str,
    /// Severity classification.
    pub kind: EventKind,
}

/// Event severity kinds for dashboard events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Informational event.
    Info,
    /// Warning event.
    Warning,
    /// Error event.
    Error,
}

/// Tracker health aggregates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerHealth {
    /// Count of healthy trackers.
    pub ok: u16,
    /// Count of warning trackers.
    pub warn: u16,
    /// Count of errored trackers.
    pub error: u16,
}

/// Queue status aggregates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueStatus {
    /// Number of active torrents.
    pub active: u16,
    /// Number of paused torrents.
    pub paused: u16,
    /// Number of queued torrents.
    pub queued: u16,
    /// Pending queue depth.
    pub depth: u16,
}

/// VPN state summary for the dashboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VpnState {
    /// Current VPN state label.
    pub state: String,
    /// Status message for the VPN.
    pub message: String,
    /// Last change timestamp.
    pub last_change: String,
}

/// Toast variants used across the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToastKind {
    /// Informational toast.
    Info,
    /// Success toast.
    Success,
    /// Error toast.
    Error,
}

/// Toast payload used by the host and app state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    /// Monotonic toast identifier.
    pub id: u64,
    /// Display message for the toast.
    pub message: String,
    /// Severity classification.
    pub kind: ToastKind,
}

/// Navigation labels supplied by the router shell.
#[derive(Clone, PartialEq, Eq)]
pub struct NavLabels {
    /// Torrents nav label.
    pub torrents: String,
    /// Categories nav label.
    pub categories: String,
    /// Tags nav label.
    pub tags: String,
    /// Settings nav label.
    pub settings: String,
    /// Health nav label.
    pub health: String,
}

/// SSE connection state shared across shell/status components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SseState {
    /// SSE connection is live.
    Connected,
    /// SSE connection is retrying.
    Reconnecting {
        /// Seconds until the next retry attempt.
        retry_in_secs: u8,
        /// Identifier for the last event seen.
        last_event: String,
        /// Human-readable reason for reconnect.
        reason: String,
    },
}

/// Dialog confirmation kinds for torrent actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmKind {
    /// Confirm deletion without data removal.
    Delete,
    /// Confirm deletion with data removal.
    DeleteData,
    /// Confirm recheck action.
    Recheck,
}

/// Torrent add payload accepted by the API and UI.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct AddTorrentInput {
    /// Magnet or URL input.
    pub value: Option<String>,
    /// Optional torrent file payload.
    pub file: Option<File>,
    /// Optional initial category.
    pub category: Option<String>,
    /// Optional initial tag list.
    pub tags: Option<Vec<String>>,
    /// Optional initial save path.
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
#[derive(Clone, Debug, PartialEq)]
pub struct FileNode {
    /// File or folder name.
    pub name: String,
    /// Total size in GB.
    pub size_gb: f64,
    /// Completed size in GB.
    pub completed_gb: f64,
    /// Priority label for the node.
    pub priority: String,
    /// Whether the node is selected for download.
    pub wanted: bool,
    /// Nested child nodes.
    pub children: Vec<FileNode>,
}

/// Peer row for detail pane.
#[derive(Clone, Debug, PartialEq)]
pub struct PeerRow {
    /// Peer IP address.
    pub ip: String,
    /// Client identification string.
    pub client: String,
    /// Peer flags string.
    pub flags: String,
    /// Country code for the peer.
    pub country: String,
    /// Download rate from the peer in bytes per second.
    pub download_bps: u64,
    /// Upload rate to the peer in bytes per second.
    pub upload_bps: u64,
    /// Completion percentage for the peer.
    pub progress: f32,
}

/// Tracker row for detail pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackerRow {
    /// Tracker announce URL.
    pub url: String,
    /// Tracker status string.
    pub status: String,
    /// Next announce time or summary.
    pub next_announce: String,
    /// Optional last error message.
    pub last_error: Option<String>,
}

/// Event row for detail pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRow {
    /// Timestamp label.
    pub timestamp: String,
    /// Severity level label.
    pub level: String,
    /// Event description.
    pub message: String,
}

/// Metadata section for detail pane.
#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    /// Info hash.
    pub hash: String,
    /// Magnet URI.
    pub magnet: String,
    /// Total size in GB.
    pub size_gb: f64,
    /// Piece count.
    pub piece_count: u32,
    /// Piece size in MB.
    pub piece_size_mb: u32,
}

/// Aggregated detail data used by the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailData {
    /// Display name.
    pub name: String,
    /// File tree listing.
    pub files: Vec<FileNode>,
    /// Peer list snapshot.
    pub peers: Vec<PeerRow>,
    /// Tracker list snapshot.
    pub trackers: Vec<TrackerRow>,
    /// Recent event log entries.
    pub events: Vec<EventRow>,
    /// Metadata summary.
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
                size_gb: bytes_to_gb(file.size_bytes),
                completed_gb: bytes_to_gb(file.completed_bytes),
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
                size_gb: bytes_to_gb(detail.size_bytes),
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
