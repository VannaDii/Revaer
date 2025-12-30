//! Shared UI view models plus re-exports of API DTOs.

pub use revaer_api_models::*;

#[cfg(target_arch = "wasm32")]
use web_sys::File;

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
    /// Dashboard nav label.
    pub dashboard: String,
    /// Torrents nav label.
    pub torrents: String,
    /// Logs nav label.
    pub logs: String,
    /// Categories nav label.
    pub categories: String,
    /// Tags nav label.
    pub tags: String,
    /// Settings nav label.
    pub settings: String,
    /// Health nav label.
    pub health: String,
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
    /// Optional download rate limit in bytes per second.
    pub max_download_bps: Option<u64>,
    /// Optional upload rate limit in bytes per second.
    pub max_upload_bps: Option<u64>,
}

#[cfg(target_arch = "wasm32")]
impl PartialEq for AddTorrentInput {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.category == other.category
            && self.tags == other.tags
            && self.save_path == other.save_path
            && self.max_download_bps == other.max_download_bps
            && self.max_upload_bps == other.max_upload_bps
            && self.file.as_ref().map(|f| f.name()) == other.file.as_ref().map(|f| f.name())
    }
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

/// Demo detail record used by the torrent view.
#[must_use]
pub fn demo_detail(id: &str) -> Option<TorrentDetail> {
    use chrono::Utc;
    use uuid::Uuid;

    let parsed = Uuid::parse_str(id).ok();
    let name = demo_detail_name(parsed);
    let id = parsed.unwrap_or_else(Uuid::nil);
    let now = Utc::now();
    Some(TorrentDetail {
        summary: demo_detail_summary(id, name, now),
        settings: Some(demo_detail_settings()),
        files: Some(demo_detail_files()),
    })
}

const DEMO_GIB: u64 = 1_073_741_824;

fn demo_detail_name(parsed: Option<uuid::Uuid>) -> &'static str {
    match parsed {
        Some(value) if value == uuid::Uuid::from_u128(2) => {
            "The.Expanse.S01E05.1080p.BluRay.DTS.x264"
        }
        Some(value) if value == uuid::Uuid::from_u128(3) => {
            "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1"
        }
        Some(value) if value == uuid::Uuid::from_u128(4) => "Ubuntu-24.04.1-live-server-amd64.iso",
        Some(value) if value == uuid::Uuid::from_u128(5) => {
            "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264"
        }
        _ => "Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10",
    }
}

fn demo_detail_summary(
    id: uuid::Uuid,
    name: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> TorrentSummary {
    let total_bytes = 18_u64 * DEMO_GIB;
    let downloaded = total_bytes.saturating_mul(41) / 100;
    TorrentSummary {
        id,
        name: Some(name.to_string()),
        state: TorrentStateView {
            kind: TorrentStateKind::Downloading,
            failure_message: None,
        },
        progress: TorrentProgressView {
            bytes_downloaded: downloaded,
            bytes_total: total_bytes,
            percent_complete: 41.0,
            eta_seconds: Some(720),
        },
        rates: TorrentRatesView {
            download_bps: 82_000_000,
            upload_bps: 1_200_000,
            ratio: 0.12,
        },
        library_path: None,
        download_dir: Some("/data/incomplete/foundation-s02e08".into()),
        sequential: false,
        tags: vec!["4K", "HDR10", "hevc"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        category: Some("tv".into()),
        trackers: vec!["tracker.hypothetical.org".into()],
        rate_limit: None,
        connections_limit: Some(200),
        added_at: now,
        completed_at: None,
        last_updated: now,
    }
}

fn demo_detail_settings() -> TorrentSettingsView {
    TorrentSettingsView {
        tags: vec!["4K".into(), "HDR10".into(), "hevc".into()],
        category: Some("tv".into()),
        trackers: vec!["tracker.hypothetical.org".into()],
        tracker_messages: std::collections::HashMap::new(),
        rate_limit: None,
        connections_limit: Some(200),
        download_dir: Some("/data/incomplete/foundation-s02e08".into()),
        comment: None,
        source: None,
        private: None,
        storage_mode: None,
        use_partfile: Some(true),
        sequential: false,
        selection: Some(TorrentSelectionView {
            include: Vec::new(),
            exclude: Vec::new(),
            skip_fluff: true,
            priorities: Vec::new(),
        }),
        super_seeding: Some(false),
        seed_mode: Some(false),
        seed_ratio_limit: None,
        seed_time_limit: None,
        cleanup: None,
        auto_managed: Some(true),
        queue_position: Some(7),
        pex_enabled: Some(true),
        web_seeds: vec!["https://cdn.example.org/foundation/".into()],
    }
}

fn demo_detail_files() -> Vec<TorrentFileView> {
    vec![
        TorrentFileView {
            index: 0,
            path: "Foundation.S02E08.mkv".to_string(),
            size_bytes: 14 * DEMO_GIB,
            bytes_completed: 6 * DEMO_GIB,
            priority: FilePriority::High,
            selected: true,
        },
        TorrentFileView {
            index: 1,
            path: "Extras/Featurette-01.mkv".to_string(),
            size_bytes: DEMO_GIB,
            bytes_completed: DEMO_GIB,
            priority: FilePriority::Normal,
            selected: true,
        },
        TorrentFileView {
            index: 2,
            path: "Extras/Interview-01.mkv".to_string(),
            size_bytes: DEMO_GIB,
            bytes_completed: 200_000_000,
            priority: FilePriority::Low,
            selected: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;
    use uuid::Uuid;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    fn test_error(message: &'static str) -> Box<dyn Error> {
        Box::new(io::Error::other(message))
    }

    #[test]
    fn demo_snapshot_populates_expected_fields() {
        let snapshot = demo_snapshot();
        assert_eq!(snapshot.download_bps, 142_000_000);
        assert_eq!(snapshot.upload_bps, 22_000_000);
        assert_eq!(snapshot.paths.len(), 3);
        assert_eq!(snapshot.recent_events.len(), 4);
        assert_eq!(snapshot.queue.depth, 34);
        assert_eq!(snapshot.vpn.state, "connected");
    }

    #[test]
    fn demo_detail_names_match_known_ids() -> Result<()> {
        let cases = [
            (
                Uuid::from_u128(2),
                "The.Expanse.S01E05.1080p.BluRay.DTS.x264",
            ),
            (
                Uuid::from_u128(3),
                "Dune.Part.One.2021.2160p.REMUX.DV.DTS-HD.MA.7.1",
            ),
            (Uuid::from_u128(4), "Ubuntu-24.04.1-live-server-amd64.iso"),
            (
                Uuid::from_u128(5),
                "Arcane.S02E02.1080p.NF.WEB-DL.DDP5.1.Atmos.x264",
            ),
        ];
        for (id, expected) in cases {
            let detail =
                demo_detail(&id.to_string()).ok_or_else(|| test_error("demo detail missing"))?;
            assert_eq!(detail.summary.name.as_deref(), Some(expected));
        }
        Ok(())
    }

    #[test]
    fn demo_detail_falls_back_on_invalid_id() -> Result<()> {
        let detail = demo_detail("not-a-uuid").ok_or_else(|| test_error("demo detail missing"))?;
        assert_eq!(detail.summary.id, Uuid::nil());
        assert_eq!(
            detail.summary.name.as_deref(),
            Some("Foundation.S02E08.2160p.WEB-DL.DDP5.1.Atmos.HDR10")
        );
        assert_eq!(detail.files.as_ref().map(Vec::len), Some(3));
        Ok(())
    }
}
