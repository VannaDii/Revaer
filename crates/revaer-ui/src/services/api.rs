//! HTTP client helpers (REST).

use crate::core::logic::build_torrents_path;
use crate::features::torrents::actions::TorrentAction;
use crate::features::torrents::state::TorrentRow;
use crate::models::{
    AddTorrentInput, DashboardSnapshot, DetailData, QueueStatus, TorrentDetail, TorrentSummary,
    TrackerHealth, VpnState,
};
use gloo_net::http::Request;
use serde::Serialize;
use web_sys::FormData;

#[derive(Clone, Debug)]
pub(crate) struct ApiClient {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl ApiClient {
    pub(crate) fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key,
        }
    }

    async fn get_json<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        let mut req = Request::get(&format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("x-revaer-api-key", key);
        }
        Ok(req.send().await?.json::<T>().await?)
    }

    async fn post_empty(&self, path: &str) -> anyhow::Result<()> {
        let mut req = Request::post(&format!("{}{}", self.base_url, path));
        if let Some(key) = &self.api_key {
            req = req.header("x-revaer-api-key", key);
        }
        req.send().await?;
        Ok(())
    }

    pub(crate) async fn perform_action(
        &self,
        id: &str,
        action: TorrentAction,
    ) -> anyhow::Result<()> {
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
                    req = req.header("x-revaer-api-key", key);
                }
                req.send().await?;
                Ok(())
            }
        }
    }

    pub(crate) async fn fetch_torrents(
        &self,
        search: Option<String>,
        regex: bool,
    ) -> anyhow::Result<Vec<TorrentRow>> {
        let data: Vec<TorrentSummary> = self.get_json(&build_torrents_path(&search, regex)).await?;
        Ok(data.into_iter().map(TorrentRow::from).collect())
    }

    pub(crate) async fn fetch_dashboard(&self) -> anyhow::Result<DashboardSnapshot> {
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
                state: "unknown".into(),
                message: "-".into(),
                last_change: "-".into(),
            },
        })
    }

    pub(crate) async fn add_torrent(&self, input: AddTorrentInput) -> anyhow::Result<TorrentRow> {
        if let Some(file) = input.file {
            self.add_torrent_file(file, input.category, input.tags, input.save_path)
                .await
        } else if let Some(source) = input.value {
            self.add_torrent_text(source, input.category, input.tags, input.save_path)
                .await
        } else {
            Err(anyhow::anyhow!("No torrent payload provided"))
        }
    }

    pub(crate) async fn fetch_torrent_detail(&self, id: &str) -> anyhow::Result<DetailData> {
        let detail: TorrentDetail = self.get_json(&format!("/v1/torrents/{id}")).await?;
        Ok(DetailData::from(detail))
    }

    async fn add_torrent_text(
        &self,
        source: String,
        category: Option<String>,
        tags: Option<Vec<String>>,
        save_path: Option<String>,
    ) -> anyhow::Result<TorrentRow> {
        #[derive(Serialize)]
        struct Body {
            source: String,
            category: Option<String>,
            tags: Option<Vec<String>>,
            save_path: Option<String>,
        }
        let mut req = Request::post(&format!(
            "{}/v1/torrents",
            self.base_url.trim_end_matches('/')
        ));
        if let Some(key) = &self.api_key {
            req = req.header("x-revaer-api-key", key);
        }
        let resp = req.json(&Body {
            source,
            category,
            tags,
            save_path,
        })?;
        Ok(resp.send().await?.json::<TorrentSummary>().await?.into())
    }

    async fn add_torrent_file(
        &self,
        file: web_sys::File,
        category: Option<String>,
        tags: Option<Vec<String>>,
        save_path: Option<String>,
    ) -> anyhow::Result<TorrentRow> {
        let form = FormData::new().map_err(|_| anyhow::anyhow!("form-data failed"))?;
        form.append_with_blob_and_filename("file", &file, &file.name())
            .map_err(|err| anyhow::anyhow!("attach file: {:?}", err))?;
        if let Some(cat) = category {
            let _ = form.append_with_str("category", &cat);
        }
        if let Some(tags) = tags {
            let _ = form.append_with_str("tags", &tags.join(","));
        }
        if let Some(path) = save_path {
            let _ = form.append_with_str("save_path", &path);
        }
        let mut req = Request::post(&format!(
            "{}/v1/torrents",
            self.base_url.trim_end_matches('/')
        ))
        .body(form);
        if let Some(key) = &self.api_key {
            req = req.header("x-revaer-api-key", key);
        }
        Ok(req.send().await?.json::<TorrentSummary>().await?.into())
    }
}
