//! HTTP and SSE client helpers (REST + fallback stubs).

use crate::components::dashboard::{
    DashboardSnapshot, PathUsage, QueueStatus, TrackerHealth, VpnState,
};
use crate::components::torrents::TorrentRow;
use crate::models::{SseEvent, TorrentSummary};
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use yew::Callback;
use yew::platform::spawn_local as yew_spawn;

#[derive(Clone, Debug)]
pub struct ApiClient {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key,
        }
    }

    async fn get_json<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        let mut req = Request::get(&format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        Ok(req.send().await?.json::<T>().await?)
    }

    pub async fn fetch_torrents(&self) -> anyhow::Result<Vec<TorrentRow>> {
        let data: Vec<TorrentSummary> = self.get_json("/v1/torrents").await?;
        Ok(data.into_iter().map(TorrentRow::from).collect())
    }

    pub async fn fetch_dashboard(&self) -> anyhow::Result<DashboardSnapshot> {
        #[derive(serde::Deserialize)]
        struct DashboardDto {
            download_bps: u64,
            upload_bps: u64,
            active: u32,
            paused: u32,
            completed: u32,
            disk_total_gb: u32,
            disk_used_gb: u32,
        }
        let dto: DashboardDto = self.get_json("/v1/dashboard").await?;
        Ok(DashboardSnapshot {
            download_bps: dto.download_bps,
            upload_bps: dto.upload_bps,
            active: dto.active,
            paused: dto.paused,
            completed: dto.completed,
            disk_total_gb: dto.disk_total_gb,
            disk_used_gb: dto.disk_used_gb,
            paths: vec![],
            recent_events: vec![],
            tracker_health: TrackerHealth {
                ok: 0,
                warn: 0,
                error: 0,
            },
            queue: QueueStatus {
                active: 0,
                paused: 0,
                queued: 0,
                depth: 0,
            },
            vpn: VpnState {
                state: "unknown",
                message: "-",
                last_change: "-",
            },
        })
    }
}

/// Handle SSE events pushed from the backend. In Phase 1 this is a placeholder that can be wired to `EventSource`.
pub fn connect_sse<F>(on_event: F)
where
    F: 'static + Fn(SseEvent) + Send + Clone,
{
    // Placeholder: simulate periodic updates until real EventSource wiring is added.
    yew_spawn(async move {
        let cb = on_event.clone();
        cb(SseEvent::SystemRates {
            download_bps: 120_000_000,
            upload_bps: 12_000_000,
        });
    });
}
