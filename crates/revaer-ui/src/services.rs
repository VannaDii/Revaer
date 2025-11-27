//! HTTP and SSE client helpers (REST + fallback stubs).

use crate::components::dashboard::{DashboardSnapshot, QueueStatus, TrackerHealth, VpnState};
use crate::components::torrents::{TorrentAction, TorrentRow};
use crate::models::{SseEvent, TorrentSummary};
use gloo_net::http::Request;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{EventSource, EventSourceInit, MessageEvent};

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

    async fn post_empty(&self, path: &str) -> anyhow::Result<()> {
        let mut req = Request::post(&format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        req.send().await?;
        Ok(())
    }

    pub async fn perform_action(&self, id: &str, action: TorrentAction) -> anyhow::Result<()> {
        match action {
            TorrentAction::Pause => self.post_empty(&format!("/v1/torrents/{id}/pause")).await,
            TorrentAction::Resume => self.post_empty(&format!("/v1/torrents/{id}/resume")).await,
            TorrentAction::Recheck => self.post_empty(&format!("/v1/torrents/{id}/recheck")).await,
            TorrentAction::Delete { with_data } => {
                let path = if with_data {
                    format!("/v1/torrents/{id}?with_data=true")
                } else {
                    format!("/v1/torrents/{id}")
                };
                let mut req = Request::delete(&format!("{}{}", self.base_url, path));
                if let Some(key) = &self.api_key {
                    req = req.header("x-api-key", key);
                }
                req.send().await?;
                Ok(())
            }
        }
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

/// Handle SSE events pushed from the backend using `EventSource`.
pub fn connect_sse(
    base_url: &str,
    api_key: Option<String>,
    on_event: impl Fn(SseEvent) + 'static,
) -> Option<EventSource> {
    let url = if let Some(key) = api_key {
        format!(
            "{}/v1/events/stream?api_key={}",
            base_url.trim_end_matches('/'),
            key
        )
    } else {
        format!("{}/v1/events/stream", base_url.trim_end_matches('/'))
    };
    let mut init = EventSourceInit::new();
    init.with_credentials(true);
    let source = EventSource::new_with_event_source_init_dict(&url, &init).ok()?;
    let handler = Closure::<dyn FnMut(_)>::wrap(Box::new(move |event: web_sys::Event| {
        if let Ok(msg) = event.dyn_into::<MessageEvent>() {
            if let Ok(text) = msg.data().dyn_into::<js_sys::JsString>() {
                if let Ok(parsed) = serde_json::from_str::<SseEvent>(&String::from(text)) {
                    on_event(parsed);
                }
            }
        }
    }) as Box<dyn FnMut(_)>);
    let _ = source.add_event_listener_with_callback("message", handler.as_ref().unchecked_ref());
    handler.forget();
    Some(source)
}
