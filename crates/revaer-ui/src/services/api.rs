//! HTTP client helpers (REST).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::core::auth::{AuthState, LocalAuth};
use crate::core::logic::build_torrents_path;
use crate::features::torrents::actions::TorrentAction as UiTorrentAction;
use crate::features::torrents::state::{TorrentsPaging, TorrentsQueryModel};
use crate::models::{
    AddTorrentInput, ApiKeyRefreshResponse, DashboardResponse, DashboardSnapshot, FsBrowseResponse,
    HealthResponse, ProblemDetails, QueueStatus, SetupCompleteResponse, SetupStartResponse,
    TorrentAction as ApiTorrentAction, TorrentAuthorRequest, TorrentAuthorResponse,
    TorrentCreateRequest, TorrentDetail, TorrentLabelEntry, TorrentListResponse,
    TorrentOptionsRequest, TorrentSelectionRequest, TrackerHealth, VpnState,
};
use base64::{Engine as _, engine::general_purpose};
use gloo::file::futures::read_as_bytes;
use gloo_net::http::{Request, RequestBuilder, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use urlencoding::encode;
use uuid::Uuid;
use web_sys::{RequestMode, window};

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

    fn apply_mode(req: RequestBuilder) -> RequestBuilder {
        req.mode(RequestMode::Cors)
    }

    fn build_request(req: RequestBuilder) -> Result<Request, ApiError> {
        req.build()
            .map_err(|_| ApiError::client("request build failed"))
    }

    fn apply_auth(&self, req: RequestBuilder) -> Result<RequestBuilder, ApiError> {
        let req = Self::apply_mode(req);
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

    async fn send_torrent_action(
        &self,
        id: &str,
        action: ApiTorrentAction,
    ) -> Result<(), ApiError> {
        let req = Request::post(&format!(
            "{}/v1/torrents/{}/action",
            self.base_url.trim_end_matches('/'),
            id
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(&action)
            .map_err(|err| ApiError::client(format!("action payload failed: {err}")))?;
        self.send_empty(req).await
    }

    pub(crate) async fn fetch_health(&self) -> Result<HealthResponse, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, "/health"));
        let req = self.apply_auth(req)?;
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn fetch_config_snapshot(&self) -> Result<Value, ApiError> {
        self.get_json("/v1/config").await
    }

    pub(crate) async fn browse_filesystem(&self, path: &str) -> Result<FsBrowseResponse, ApiError> {
        let encoded = encode(path);
        let url = format!(
            "{}/v1/fs/browse?path={}",
            self.base_url.trim_end_matches('/'),
            encoded
        );
        let req = Request::get(&url);
        let req = self.apply_auth(req)?;
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn patch_settings(&self, changeset: Value) -> Result<Value, ApiError> {
        let req = Request::patch(&format!(
            "{}/v1/config",
            self.base_url.trim_end_matches('/')
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(&changeset)
            .map_err(|err| ApiError::client(format!("settings payload failed: {err}")))?;
        self.send_json(req).await
    }

    pub(crate) async fn setup_start(&self) -> Result<SetupStartResponse, ApiError> {
        let req = Self::apply_mode(Request::post(&format!(
            "{}{}",
            self.base_url, "/admin/setup/start"
        )));
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn setup_complete(
        &self,
        token: &str,
        changeset: Value,
    ) -> Result<SetupCompleteResponse, ApiError> {
        let mut req = Self::apply_mode(Request::post(&format!(
            "{}{}",
            self.base_url, "/admin/setup/complete"
        )));
        req = req.header("x-revaer-setup-token", token);
        let req = req
            .json(&changeset)
            .map_err(|err| ApiError::client(format!("setup payload failed: {err}")))?;
        self.send_json::<SetupCompleteResponse>(req).await
    }

    pub(crate) async fn fetch_well_known_snapshot(&self) -> Result<Value, ApiError> {
        let req = Self::apply_mode(Request::get(&format!(
            "{}{}",
            self.base_url, "/.well-known/revaer.json"
        )));
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn refresh_api_key(&self) -> Result<ApiKeyRefreshResponse, ApiError> {
        let req = Request::post(&format!("{}{}", self.base_url, "/v1/auth/refresh"));
        let req = self.apply_auth(req)?;
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn factory_reset(&self, confirm: &str) -> Result<(), ApiError> {
        let mut req = Request::post(&format!("{}{}", self.base_url, "/admin/factory-reset"));
        req = self.apply_auth(req)?;
        let req = req
            .json(&json!({ "confirm": confirm }))
            .map_err(|err| ApiError::client(format!("factory reset payload failed: {err}")))?;
        self.send_empty(req).await
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, ApiError> {
        let req = Request::get(&format!("{}{}", self.base_url, path));
        let req = self.apply_auth(req)?;
        let req = Self::build_request(req)?;
        self.send_json(req).await
    }

    pub(crate) async fn fetch_categories(&self) -> Result<Vec<TorrentLabelEntry>, ApiError> {
        self.get_json("/v1/torrents/categories").await
    }

    pub(crate) async fn fetch_tags(&self) -> Result<Vec<TorrentLabelEntry>, ApiError> {
        self.get_json("/v1/torrents/tags").await
    }

    pub(crate) async fn perform_action(
        &self,
        id: &str,
        action: UiTorrentAction,
    ) -> Result<(), ApiError> {
        match action {
            UiTorrentAction::Pause | UiTorrentAction::Resume => {
                let paused = matches!(action, UiTorrentAction::Pause);
                let request = TorrentOptionsRequest {
                    paused: Some(paused),
                    ..TorrentOptionsRequest::default()
                };
                self.update_torrent_options(id, &request).await
            }
            UiTorrentAction::Reannounce => {
                self.send_torrent_action(id, ApiTorrentAction::Reannounce)
                    .await
            }
            UiTorrentAction::Recheck => {
                self.send_torrent_action(id, ApiTorrentAction::Recheck)
                    .await
            }
            UiTorrentAction::Sequential { enable } => {
                self.send_torrent_action(id, ApiTorrentAction::Sequential { enable })
                    .await
            }
            UiTorrentAction::Rate {
                download_bps,
                upload_bps,
            } => {
                if download_bps.is_none() && upload_bps.is_none() {
                    return Err(ApiError::client(
                        "rate action requires download or upload value",
                    ));
                }
                self.send_torrent_action(
                    id,
                    ApiTorrentAction::Rate {
                        download_bps,
                        upload_bps,
                    },
                )
                .await
            }
            UiTorrentAction::Delete { with_data } => {
                self.send_torrent_action(
                    id,
                    ApiTorrentAction::Remove {
                        delete_data: with_data,
                    },
                )
                .await
            }
        }
    }

    pub(crate) async fn fetch_torrents(
        &self,
        filters: &TorrentsQueryModel,
        paging: &TorrentsPaging,
    ) -> Result<TorrentListResponse, ApiError> {
        self.get_json(&build_torrents_path(filters, paging)).await
    }

    pub(crate) async fn fetch_dashboard(&self) -> Result<DashboardSnapshot, ApiError> {
        let dto: DashboardResponse = self.get_json("/v1/dashboard").await?;
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
        let AddTorrentInput {
            value,
            file,
            category,
            tags,
            save_path,
            max_download_bps,
            max_upload_bps,
        } = input;
        if let Some(file) = file {
            self.add_torrent_file(
                file,
                category,
                tags,
                save_path,
                max_download_bps,
                max_upload_bps,
            )
            .await
        } else if let Some(source) = value {
            self.add_torrent_text(
                source,
                category,
                tags,
                save_path,
                max_download_bps,
                max_upload_bps,
            )
            .await
        } else {
            Err(ApiError::client("no torrent payload provided"))
        }
    }

    pub(crate) async fn create_torrent(
        &self,
        request: &TorrentAuthorRequest,
    ) -> Result<TorrentAuthorResponse, ApiError> {
        let req = Request::post(&format!(
            "{}/v1/torrents/create",
            self.base_url.trim_end_matches('/')
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(request)
            .map_err(|err| ApiError::client(format!("encode create payload: {err}")))?;
        self.send_json(req).await
    }

    pub(crate) async fn fetch_torrent_detail(&self, id: &str) -> Result<TorrentDetail, ApiError> {
        self.get_json(&format!("/v1/torrents/{id}")).await
    }

    pub(crate) async fn update_torrent_options(
        &self,
        id: &str,
        request: &TorrentOptionsRequest,
    ) -> Result<(), ApiError> {
        let req = Request::patch(&format!(
            "{}/v1/torrents/{}/options",
            self.base_url.trim_end_matches('/'),
            id
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(request)
            .map_err(|err| ApiError::client(format!("encode options payload: {err}")))?;
        self.send_empty(req).await
    }

    pub(crate) async fn update_torrent_selection(
        &self,
        id: &str,
        request: &TorrentSelectionRequest,
    ) -> Result<(), ApiError> {
        let req = Request::post(&format!(
            "{}/v1/torrents/{}/select",
            self.base_url.trim_end_matches('/'),
            id
        ));
        let req = self.apply_auth(req)?;
        let req = req
            .json(request)
            .map_err(|err| ApiError::client(format!("encode selection payload: {err}")))?;
        self.send_empty(req).await
    }

    async fn add_torrent_text(
        &self,
        source: String,
        category: Option<String>,
        tags: Option<Vec<String>>,
        save_path: Option<String>,
        max_download_bps: Option<u64>,
        max_upload_bps: Option<u64>,
    ) -> Result<Uuid, ApiError> {
        let id = Uuid::new_v4();
        let request = TorrentCreateRequest {
            id,
            magnet: Some(source),
            download_dir: save_path,
            tags: tags.unwrap_or_default(),
            category,
            max_download_bps,
            max_upload_bps,
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
        max_download_bps: Option<u64>,
        max_upload_bps: Option<u64>,
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
            max_download_bps,
            max_upload_bps,
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
        let detail = if let Some(params) = problem
            .invalid_params
            .as_ref()
            .filter(|items| !items.is_empty())
        {
            if params.len() == 1 {
                Some(params[0].message.clone())
            } else {
                Some(
                    params
                        .iter()
                        .map(|param| format!("{}: {}", param.pointer, param.message))
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            }
        } else if let Some(fields) = problem.context.as_ref().filter(|items| !items.is_empty()) {
            if fields.len() == 1 {
                Some(fields[0].value.clone())
            } else {
                Some(
                    fields
                        .iter()
                        .map(|field| format!("{}: {}", field.name, field.value))
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            }
        } else {
            problem.detail.clone()
        };
        return ApiError {
            status: problem.status,
            title: problem.title,
            detail,
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
