//! qBittorrent compatibility façade (`/api/v2`).
//!
//! The façade maps Revaer's domain model onto the subset of qBittorrent
//! endpoints needed for Phase 1 interoperability with existing clients. The
//! implementation intentionally keeps the surface minimal while remaining
//! conservative about authentication (no-op login) until the full auth model
//! lands in a later phase.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Form, Query, State},
    http::{
        HeaderMap, HeaderValue,
        header::{CONTENT_TYPE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use uuid::Uuid;

use revaer_events::TorrentState;
use revaer_torrent_core::{RemoveTorrent, TorrentRateLimit, TorrentStatus};

use crate::{
    ApiError, ApiState, TorrentHandles, TorrentMetadata, dispatch_torrent_add,
    models::TorrentCreateRequest,
};

/// Attach qBittorrent-compatible endpoints to the primary router.
pub fn mount(router: Router<Arc<ApiState>>) -> Router<Arc<ApiState>> {
    router
        .route("/api/v2/auth/login", post(login))
        .route("/api/v2/app/version", get(app_version))
        .route("/api/v2/app/webapiVersion", get(app_webapi_version))
        .route("/api/v2/sync/maindata", get(sync_maindata))
        .route("/api/v2/torrents/info", get(torrents_info))
        .route("/api/v2/torrents/add", post(torrents_add))
        .route("/api/v2/torrents/pause", post(torrents_pause))
        .route("/api/v2/torrents/resume", post(torrents_resume))
        .route("/api/v2/torrents/delete", post(torrents_delete))
        .route("/api/v2/transfer/uploadlimit", post(transfer_upload_limit))
        .route(
            "/api/v2/transfer/downloadlimit",
            post(transfer_download_limit),
        )
}

#[derive(Deserialize, Default)]
struct LoginForm {
    #[allow(dead_code)]
    username: Option<String>,
    #[allow(dead_code)]
    password: Option<String>,
}

async fn login(Form(_): Form<LoginForm>) -> Result<impl IntoResponse, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_static("SID=revaer-session; HttpOnly; Path=/"),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    Ok((headers, "Ok."))
}

async fn app_version() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (headers, format!("Revaer {}", env!("CARGO_PKG_VERSION")))
}

async fn app_webapi_version() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (headers, "2.8.18")
}

#[derive(Deserialize, Default)]
pub struct SyncParams {
    #[allow(dead_code)]
    pub rid: Option<u64>,
}

#[derive(Serialize)]
pub struct SyncMainData {
    pub full_update: bool,
    pub rid: u64,
    pub torrents: HashMap<String, QbTorrentEntry>,
    pub categories: HashMap<String, QbCategory>,
    pub server_state: QbServerState,
}

#[derive(Serialize, Default)]
pub struct QbCategory {
    pub name: String,
    pub save_path: String,
}

#[derive(Serialize)]
pub struct QbServerState {
    pub dl_info_speed: i64,
    pub up_info_speed: i64,
    pub dl_rate_limit: i64,
    pub up_rate_limit: i64,
    pub dl_info_data: i64,
    pub up_info_data: i64,
    pub queueing: bool,
}

#[derive(Serialize)]
pub struct QbTorrentEntry {
    pub added_on: i64,
    pub completion_on: i64,
    pub category: String,
    pub dlspeed: i64,
    pub upspeed: i64,
    pub downloaded: i64,
    pub uploaded: i64,
    pub size: i64,
    pub progress: f64,
    pub state: String,
    pub name: String,
    pub hash: String,
    pub save_path: String,
    #[serde(rename = "seq_dl")]
    pub sequential_download: bool,
    #[serde(rename = "eta")]
    pub eta_seconds: i64,
    pub ratio: f64,
    #[serde(rename = "tags")]
    pub tag_list: String,
}

pub async fn sync_maindata(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<SyncParams>,
) -> Result<Json<SyncMainData>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let statuses = handles.inspector().list().await.map_err(|err| {
        error!(error = %err, "failed to list torrents for qB sync");
        ApiError::internal("failed to list torrents")
    })?;

    let mut torrents = HashMap::new();
    for status in statuses {
        let metadata = state.get_metadata(&status.id);
        let entry = qb_entry(&status, &metadata);
        torrents.insert(entry.hash.clone(), entry);
    }

    let baseline_rid = params.rid.unwrap_or(0);
    let rid = state.events.last_event_id().unwrap_or(baseline_rid);
    let server_state = build_server_state(torrents.values().map(|entry| {
        (
            entry.dlspeed,
            entry.upspeed,
            entry.downloaded,
            entry.uploaded,
        )
    }));

    Ok(Json(SyncMainData {
        full_update: true,
        rid,
        torrents,
        categories: HashMap::new(),
        server_state,
    }))
}

#[derive(Deserialize, Default, Clone)]
pub struct TorrentsInfoParams {
    pub hashes: Option<String>,
}

pub async fn torrents_info(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<TorrentsInfoParams>,
) -> Result<Json<Vec<QbTorrentEntry>>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let statuses = handles.inspector().list().await.map_err(|err| {
        error!(error = %err, "failed to list torrents for qB info");
        ApiError::internal("failed to list torrents")
    })?;

    let filter_hashes = params
        .hashes
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(split_hashes);

    let mut results = Vec::new();
    for status in statuses {
        let hash = status.id.simple().to_string();
        if let Some(ref filter) = filter_hashes {
            let include_all = filter.iter().any(|entry| entry.eq_ignore_ascii_case("all"));
            if !include_all && !filter.contains(&hash) {
                continue;
            }
        }
        let metadata = state.get_metadata(&status.id);
        results.push(qb_entry(&status, &metadata));
    }

    Ok(Json(results))
}

#[derive(Deserialize, Default, Clone)]
pub struct TorrentAddForm {
    pub urls: Option<String>,
    pub tags: Option<String>,
    #[serde(rename = "savepath")]
    pub save_path: Option<String>,
    #[serde(rename = "sequentialDownload")]
    pub sequential: Option<bool>,
}

pub async fn torrents_add(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TorrentAddForm>,
) -> Result<Response, ApiError> {
    let Some(urls) = form
        .urls
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return Err(ApiError::bad_request("urls parameter is required"));
    };

    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let tags = parse_tags(form.tags.as_deref());
    let mut added = 0usize;

    for magnet in urls.lines().map(str::trim).filter(|v| !v.is_empty()) {
        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some(magnet.to_string()),
            download_dir: form.save_path.clone(),
            sequential: form.sequential,
            tags: tags.clone(),
            ..TorrentCreateRequest::default()
        };

        dispatch_torrent_add(Some(handles), &request).await?;
        state.set_metadata(
            request.id,
            TorrentMetadata::new(request.tags.clone(), request.trackers.clone()),
        );
        added += 1;
    }

    if added == 0 {
        return Err(ApiError::bad_request("no valid torrent URLs provided"));
    }

    state.update_torrent_metrics().await;

    Ok(ok_plain())
}

#[derive(Deserialize, Default, Clone)]
pub struct TorrentHashesForm {
    pub hashes: String,
}

pub async fn torrents_pause(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TorrentHashesForm>,
) -> Result<Response, ApiError> {
    apply_to_hashes(&state, &form.hashes, |workflow, id| async move {
        workflow.pause_torrent(id).await
    })
    .await?;
    Ok(ok_plain())
}

pub async fn torrents_resume(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TorrentHashesForm>,
) -> Result<Response, ApiError> {
    apply_to_hashes(&state, &form.hashes, |workflow, id| async move {
        workflow.resume_torrent(id).await
    })
    .await?;
    Ok(ok_plain())
}

#[derive(Deserialize, Default, Clone)]
pub struct TorrentDeleteForm {
    pub hashes: String,
    #[serde(rename = "deleteFiles")]
    pub delete_files: Option<bool>,
}

pub async fn torrents_delete(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TorrentDeleteForm>,
) -> Result<Response, ApiError> {
    let delete_data = form.delete_files.unwrap_or(false);
    apply_to_hashes(&state, &form.hashes, |workflow, id| async move {
        workflow
            .remove_torrent(
                id,
                RemoveTorrent {
                    with_data: delete_data,
                },
            )
            .await
    })
    .await?;
    state.update_torrent_metrics().await;
    Ok(ok_plain())
}

#[derive(Deserialize, Default, Clone)]
pub struct TransferLimitForm {
    pub limit: String,
}

pub async fn transfer_upload_limit(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TransferLimitForm>,
) -> Result<Response, ApiError> {
    apply_rate_limit(&state, None, parse_limit(&form.limit)?).await?;
    Ok(ok_plain())
}

pub async fn transfer_download_limit(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<TransferLimitForm>,
) -> Result<Response, ApiError> {
    apply_rate_limit(&state, parse_limit(&form.limit)?, None).await?;
    Ok(ok_plain())
}

async fn apply_rate_limit(
    state: &Arc<ApiState>,
    download_bps: Option<u64>,
    upload_bps: Option<u64>,
) -> Result<(), ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let workflow = handles.workflow();

    let limits = TorrentRateLimit {
        download_bps,
        upload_bps,
    };

    workflow.update_limits(None, limits).await.map_err(|err| {
        error!(error = %err, "failed to apply global rate limit");
        ApiError::internal("failed to apply rate limit")
    })
}

async fn apply_to_hashes<Fut, F>(
    state: &Arc<ApiState>,
    hashes: &str,
    mut op: F,
) -> Result<(), ApiError>
where
    F: FnMut(Arc<dyn revaer_torrent_core::TorrentWorkflow>, Uuid) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let workflow = Arc::clone(handles.workflow());

    let ids = resolve_hashes(handles, hashes).await?;
    for id in ids {
        op(Arc::clone(&workflow), id).await.map_err(|err| {
            error!(error = %err, torrent_id = %id, "torrent command failed");
            ApiError::internal("torrent command failed")
        })?;
    }

    Ok(())
}

async fn resolve_hashes(handles: &TorrentHandles, hashes: &str) -> Result<Vec<Uuid>, ApiError> {
    if hashes.eq_ignore_ascii_case("all") {
        let statuses = handles.inspector().list().await.map_err(|err| {
            error!(error = %err, "failed to list torrents for hash resolution");
            ApiError::internal("failed to list torrents")
        })?;
        return Ok(statuses.into_iter().map(|status| status.id).collect());
    }

    let mut ids = Vec::new();
    for hash in split_hashes(hashes) {
        if hash.is_empty() {
            continue;
        }
        match Uuid::parse_str(hash.as_str()) {
            Ok(id) => ids.push(id),
            Err(_) => {
                return Err(ApiError::bad_request(format!(
                    "invalid torrent hash '{hash}'"
                )));
            }
        }
    }
    Ok(ids)
}

fn qb_entry(status: &TorrentStatus, metadata: &TorrentMetadata) -> QbTorrentEntry {
    let hash = status.id.simple().to_string();
    let name = status.name.clone().unwrap_or_else(|| hash.clone());
    let added_on = status.added_at.timestamp();
    let completion_on = status
        .completed_at
        .map(|dt| dt.timestamp())
        .unwrap_or_default();
    let progress = progress_fraction(status);
    let state = qb_state(&status.state);
    let download_dir = status.download_dir.clone();
    let library_path = status.library_path.clone();
    let save_path = download_dir.or(library_path).unwrap_or_default();

    let eta_seconds = status
        .progress
        .eta_seconds
        .map_or(-1, |eta| i64::try_from(eta).unwrap_or(-1));

    let tag_list = metadata.tags.join(",");

    QbTorrentEntry {
        added_on,
        completion_on,
        category: String::new(),
        dlspeed: i64::try_from(status.rates.download_bps).unwrap_or(i64::MAX),
        upspeed: i64::try_from(status.rates.upload_bps).unwrap_or(i64::MAX),
        downloaded: i64::try_from(status.progress.bytes_downloaded).unwrap_or(i64::MAX),
        uploaded: 0,
        size: i64::try_from(status.progress.bytes_total).unwrap_or(i64::MAX),
        progress,
        state: state.to_string(),
        name,
        hash,
        save_path,
        sequential_download: status.sequential,
        eta_seconds,
        ratio: status.rates.ratio,
        tag_list,
    }
}

const fn qb_state(state: &TorrentState) -> &'static str {
    match state {
        TorrentState::Queued => "queuedDL",
        TorrentState::FetchingMetadata => "metaDL",
        TorrentState::Downloading => "downloading",
        TorrentState::Seeding => "uploading",
        TorrentState::Completed => "stalledUP",
        TorrentState::Failed { .. } => "error",
        TorrentState::Stopped => "pausedDL",
    }
}

#[allow(clippy::cast_precision_loss)]
fn progress_fraction(status: &TorrentStatus) -> f64 {
    if status.progress.bytes_total == 0 {
        0.0
    } else {
        (status.progress.bytes_downloaded as f64) / (status.progress.bytes_total as f64)
    }
}

fn build_server_state<I>(iter: I) -> QbServerState
where
    I: Iterator<Item = (i64, i64, i64, i64)>,
{
    let mut dl_speed = 0i64;
    let mut up_speed = 0i64;
    let mut dl_data = 0i64;
    let mut up_data = 0i64;

    for (down, up, downloaded, uploaded) in iter {
        dl_speed = dl_speed.saturating_add(down);
        up_speed = up_speed.saturating_add(up);
        dl_data = dl_data.saturating_add(downloaded);
        up_data = up_data.saturating_add(uploaded);
    }

    QbServerState {
        dl_info_speed: dl_speed,
        up_info_speed: up_speed,
        dl_rate_limit: -1,
        up_rate_limit: -1,
        dl_info_data: dl_data,
        up_info_data: up_data,
        queueing: false,
    }
}

fn parse_tags(value: Option<&str>) -> Vec<String> {
    value
        .map(|raw| {
            raw.split(&[',', ';'][..])
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn split_hashes(input: &str) -> Vec<String> {
    input
        .split('|')
        .map(|entry| entry.trim().to_string())
        .collect()
}

#[allow(clippy::cast_sign_loss)]
pub fn parse_limit(raw: &str) -> Result<Option<u64>, ApiError> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("NaN") || trimmed.is_empty() {
        return Ok(None);
    }
    let value: i64 = trimmed.parse().map_err(|_| {
        warn!(limit = %trimmed, "invalid limit parameter");
        ApiError::bad_request("limit must be an integer")
    })?;
    if value <= 0 {
        Ok(None)
    } else {
        Ok(Some(value as u64))
    }
}

fn ok_plain() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (headers, "Ok.").into_response()
}
