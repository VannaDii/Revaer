//! HTTP client helpers (REST).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::core::auth::{AuthState, LocalAuth};
use crate::core::logic::build_torrents_path;
use crate::features::torrents::actions::TorrentAction as UiTorrentAction;
use crate::features::torrents::state::{TorrentsPaging, TorrentsQueryModel};
use crate::models::{
    AddTorrentInput, DashboardSnapshot, DetailData, ProblemDetails, QueueStatus,
    TorrentAction as ApiTorrentAction, TorrentCreateRequest, TorrentDetail, TorrentLabelEntry,
    TorrentLabelPolicy, TorrentListResponse, TrackerHealth, VpnState,
};
use base64::{Engine as _, engine::general_purpose};
use gloo::file::futures::read_as_bytes;
use gloo_net::http::{Request, Response};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use web_sys::window;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApiError {
    pub status: u16,
    pub title: String,
    pub detail: Option<String>,
    pub retry_after_secs: Option<u64>,
}

impl ApiError {
    fn client(detail: impl Into<String>) -> Self {
        Self {
            status: 0,
            title: "client_error".to_string(),
            detail: Some(detail.into()),
            retry_after_secs: None,
        }
    }

    pub(crate) const fn is_rate_limited(&self) -> bool {
        self.status == 429
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.status, self.retry_after_secs, &self.detail) {
            (429, Some(delay), Some(detail)) => {
                write!(f, "429 {} (retry in {}s): {}", self.title, delay, detail)
            }
            (429, Some(delay), None) => {
                write!(f, "429 {} (retry in {}s)", self.title, delay)
            }
            (_, _, Some(detail)) if self.status != 0 => {
                write!(f, "{} {}: {}", self.status, self.title, detail)
            }
            (_, _, _) if self.status != 0 => write!(f, "{} {}", self.status, self.title),
            _ => write!(f, "{}", self.detail.as_deref().unwrap_or("client error")),
        }
    }
}

impl std::error::Error for ApiError {}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HealthComponentResponse {
    pub status: String,
    pub revision: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HealthResponse {
    pub status: String,
    pub mode: String,
    pub database: HealthComponentResponse,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HealthMetricsResponse {
    pub config_watch_latency_ms: i64,
    pub config_apply_latency_ms: i64,
    pub config_update_failures_total: u64,
    pub config_watch_slow_total: u64,
    pub guardrail_violations_total: u64,
    pub rate_limit_throttled_total: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct TorrentHealthResponse {
    pub active: i64,
    pub queue_depth: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FullHealthResponse {
    pub status: String,
    pub mode: String,
    pub revision: i64,
    pub build: String,
    pub degraded: Vec<String>,
    pub metrics: HealthMetricsResponse,
    pub torrent: TorrentHealthResponse,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SetupStartResponse {
    pub token: String,
    pub expires_at: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ApiClient {
    base_url: String,
    auth: Rc<RefCell<Option<AuthState>>>,
}

impl ApiClient {
    pub(crate) fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            auth: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn set_auth(&self, auth: Option<AuthState>) {
        *self.auth.borrow_mut() = auth;
    }

    fn apply_auth(&self, req: Request) -> Result<Request, ApiError> {
        match self.auth.borrow().as_ref() {
            Some(AuthState::ApiKey(key)) if !key.trim().is_empty() => {
                Ok(req.header("x-revaer-api-key", key))
            }
            Some(AuthState::Local(auth)) => {
                let header = basic_auth_header(auth)?;
                Ok(req.header("Authorization", &header))
            }
            _ => Ok(req),
        }
    }

    async fn send_json<T: for<'de> Deserialize<'de>>(&self, req: Request) -> Result<T, ApiError> {
        let response = req
            .send()
            .await
            .map_err(|err| ApiError::client(format!("request failed: {err}")))?;
        if response.ok() {
            response
                .json::<T>()
                .await
                .map_err(|err| ApiError::client(format!("invalid JSON: {err}")))
        } else {
            Err(api_error_from_response(response).await)
        }
    }

    async fn send_empty(&self, req: Request) -> Result<(), ApiError> {
        let response = req
            .send()
            .await
            .map_err(|err| ApiError::client(format!("request failed: {err}")))?;
        if response.ok() {
            Ok(())
        } else {
            Err(api_error_from_response(response).await)
        }
    }

    async fn send_text(&self, req: Request) -> Result<String, ApiError> {
        let response = req
            .send()
            .await
            .map_err(|err| ApiError::client(format!("request failed: {err}")))?;
        if response.ok() {
            response
                .text()
                .await
                .map_err(|err| ApiError::client(format!("invalid text: {err}")))
        } else {
            Err(api_error_from_response(response).await)
        }
    }

    pub(crate) async fn fetch_health(&self) -> Result<HealthResponse, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, "/health"));
        self.send_json(req).await
    }

    pub(crate) async fn fetch_health_full(&self) -> Result<FullHealthResponse, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, "/health/full"));
        self.send_json(req).await
    }

    pub(crate) async fn fetch_metrics(&self) -> Result<String, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, "/metrics"));
        let req = self.apply_auth(req)?;
        self.send_text(req).await
    }

    pub(crate) async fn setup_start(&self) -> Result<SetupStartResponse, ApiError> {
        let req = Request::post(&format!("{}{}", self.base_url, "/admin/setup/start"));
        self.send_json(req).await
    }

    pub(crate) async fn setup_complete(&self, token: &str) -> Result<(), ApiError> {
        let mut req = Request::post(&format!("{}{}", self.base_url, "/admin/setup/complete"));
        req = req.header("x-revaer-setup-token", token);
        let req = req
            .json(&json!({}))
            .map_err(|err| ApiError::client(format!("setup payload failed: {err}")))?;
        self.send_empty(req).await
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, path));
        let req = self.apply_auth(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn fetch_categories(&self) -> Result<Vec<TorrentLabelEntry>, ApiError> {
        self.get_json("/v1/torrents/categories").await
    }

    pub(crate) async fn fetch_tags(&self) -> Result<Vec<TorrentLabelEntry>, ApiError> {
        self.get_json("/v1/torrents/tags").await
    }

    pub(crate) async fn upsert_category(
        &self,
        name: &str,
        policy: &TorrentLabelPolicy,
    ) -> Result<TorrentLabelEntry, ApiError> {
        let encoded = urlencoding::encode(name);
        let req = Request::put(&format!(
            "{}/v1/torrents/categories/{}",
            self.base_url.trim_end_matches('/'),
            encoded
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(policy)
            .map_err(|err| ApiError::client(format!("encode category policy: {err}")))?;
        self.send_json(req).await
    }

    pub(crate) async fn upsert_tag(
        &self,
        name: &str,
        policy: &TorrentLabelPolicy,
    ) -> Result<TorrentLabelEntry, ApiError> {
        let encoded = urlencoding::encode(name);
        let req = Request::put(&format!(
            "{}/v1/torrents/tags/{}",
            self.base_url.trim_end_matches('/'),
            encoded
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(policy)
            .map_err(|err| ApiError::client(format!("encode tag policy: {err}")))?;
        self.send_json(req).await
    }

    pub(crate) async fn perform_action(
        &self,
        id: &str,
        action: UiTorrentAction,
    ) -> Result<(), ApiError> {
        let api_action = match action {
            UiTorrentAction::Pause => ApiTorrentAction::Pause,
            UiTorrentAction::Resume => ApiTorrentAction::Resume,
            UiTorrentAction::Recheck => ApiTorrentAction::Recheck,
            UiTorrentAction::Delete { with_data } => ApiTorrentAction::Remove {
                delete_data: with_data,
            },
        };
        let req = Request::post(&format!(
            "{}/v1/torrents/{}/action",
            self.base_url.trim_end_matches('/'),
            id
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(&api_action)
            .map_err(|err| ApiError::client(format!("action payload failed: {err}")))?;
        self.send_empty(req).await
    }

    pub(crate) async fn fetch_torrents(
        &self,
        filters: &TorrentsQueryModel,
        paging: &TorrentsPaging,
    ) -> Result<TorrentListResponse, ApiError> {
        self.get_json(&build_torrents_path(filters, paging)).await
    }

    pub(crate) async fn fetch_dashboard(&self) -> Result<DashboardSnapshot, ApiError> {
        #[derive(Deserialize)]
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

    pub(crate) async fn add_torrent(&self, input: AddTorrentInput) -> Result<Uuid, ApiError> {
        if let Some(file) = input.file {
            self.add_torrent_file(file, input.category, input.tags, input.save_path)
                .await
        } else if let Some(source) = input.value {
            self.add_torrent_text(source, input.category, input.tags, input.save_path)
                .await
        } else {
            Err(ApiError::client("no torrent payload provided"))
        }
    }

    pub(crate) async fn fetch_torrent_detail(&self, id: &str) -> Result<DetailData, ApiError> {
        let detail: TorrentDetail = self.get_json(&format!("/v1/torrents/{id}")).await?;
        Ok(DetailData::from(detail))
    }

    async fn add_torrent_text(
        &self,
        source: String,
        category: Option<String>,
        tags: Option<Vec<String>>,
        save_path: Option<String>,
    ) -> Result<Uuid, ApiError> {
        let id = Uuid::new_v4();
        let request = TorrentCreateRequest {
            id,
            magnet: Some(source),
            download_dir: save_path,
            tags: tags.unwrap_or_default(),
            category,
            ..TorrentCreateRequest::default()
        };
        let req = Request::post(&format!(
            "{}/v1/torrents",
            self.base_url.trim_end_matches('/')
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(&request)
            .map_err(|err| ApiError::client(format!("encode add payload: {err}")))?;
        self.send_empty(req).await?;
        Ok(id)
    }

    async fn add_torrent_file(
        &self,
        file: web_sys::File,
        category: Option<String>,
        tags: Option<Vec<String>>,
        save_path: Option<String>,
    ) -> Result<Uuid, ApiError> {
        let id = Uuid::new_v4();
        let blob = gloo::file::Blob::from(file);
        let bytes = read_as_bytes(&blob)
            .await
            .map_err(|err| ApiError::client(format!("read torrent file: {err}")))?;
        let metainfo = general_purpose::STANDARD.encode(bytes);
        let request = TorrentCreateRequest {
            id,
            metainfo: Some(metainfo),
            download_dir: save_path,
            tags: tags.unwrap_or_default(),
            category,
            ..TorrentCreateRequest::default()
        };
        let req = Request::post(&format!(
            "{}/v1/torrents",
            self.base_url.trim_end_matches('/')
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(&request)
            .map_err(|err| ApiError::client(format!("encode add payload: {err}")))?;
        self.send_empty(req).await?;
        Ok(id)
    }
}

fn basic_auth_header(auth: &LocalAuth) -> Result<String, ApiError> {
    let raw = format!("{}:{}", auth.username, auth.password);
    let window = window().ok_or_else(|| ApiError::client("window unavailable"))?;
    let encoded = window
        .btoa(&raw)
        .map_err(|_| ApiError::client("basic auth encoding failed"))?;
    Ok(format!("Basic {}", encoded))
}

async fn api_error_from_response(response: Response) -> ApiError {
    let retry_after = response
        .headers()
        .get("Retry-After")
        .and_then(|value| value.parse::<u64>().ok());
    if let Ok(problem) = response.json::<ProblemDetails>().await {
        return ApiError {
            status: problem.status,
            title: problem.title,
            detail: problem.detail,
            retry_after_secs: retry_after,
        };
    }

    let detail = response.text().await.ok().filter(|text| !text.is_empty());
    ApiError {
        status: response.status(),
        title: response.status_text(),
        detail,
        retry_after_secs: retry_after,
    }
}
