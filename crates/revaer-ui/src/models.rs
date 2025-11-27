//! Shared DTOs matching the Phase 1 API and SSE payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorrentSummary {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub progress: f32,
    pub eta_seconds: Option<u64>,
    pub ratio: f32,
    pub tags: Vec<String>,
    pub tracker: Option<String>,
    pub save_path: Option<String>,
    pub category: Option<String>,
    pub size_bytes: u64,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub added_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TorrentFile {
    pub path: String,
    pub size_bytes: u64,
    pub completed_bytes: u64,
    pub priority: String,
    pub wanted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    pub ip: String,
    pub client: String,
    pub flags: String,
    pub country: Option<String>,
    pub download_bps: u64,
    pub upload_bps: u64,
    pub progress: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tracker {
    pub url: String,
    pub status: String,
    pub next_announce_at: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum SseEvent {
    #[serde(rename = "torrent_progress")]
    TorrentProgress {
        torrent_id: Uuid,
        progress: f32,
        eta_seconds: Option<u64>,
        download_bps: u64,
        upload_bps: u64,
    },
    #[serde(rename = "torrent_state")]
    TorrentState {
        torrent_id: Uuid,
        status: String,
        reason: Option<String>,
    },
    #[serde(rename = "torrent_rates")]
    TorrentRates {
        torrent_id: Uuid,
        download_bps: u64,
        upload_bps: u64,
    },
    #[serde(rename = "torrent_added")]
    TorrentAdded { torrent_id: Uuid },
    #[serde(rename = "torrent_removed")]
    TorrentRemoved { torrent_id: Uuid },
    #[serde(rename = "jobs_update")]
    JobsUpdate { jobs: Vec<Job> },
    #[serde(rename = "vpn_state")]
    VpnState {
        state: String,
        message: String,
        last_change: String,
    },
    #[serde(rename = "system_rates")]
    SystemRates { download_bps: u64, upload_bps: u64 },
    #[serde(rename = "queue_status")]
    QueueStatus {
        active: u32,
        paused: u32,
        queued: u32,
        depth: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub torrent_id: Option<Uuid>,
    pub kind: String,
    pub status: String,
    pub detail: Option<String>,
    pub updated_at: String,
}
