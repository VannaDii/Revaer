//! qBittorrent compatibility façade (`/api/v2`).
//!
//! The façade maps Revaer's domain model onto the subset of qBittorrent
//! endpoints needed for Phase 1 interoperability with existing clients. The
//! implementation intentionally keeps the surface minimal while remaining
//! conservative about authentication (no-op login) until the full auth model
//! lands in a later phase.

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Form, Query, State},
    http::{
        HeaderMap, HeaderValue,
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, warn};
use uuid::Uuid;

use revaer_events::{Event as CoreEvent, TorrentState};
use revaer_torrent_core::{RemoveTorrent, TorrentLabelPolicy, TorrentRateLimit, TorrentStatus};

use crate::app::state::{ApiState, COMPAT_SESSION_TTL};
use crate::http::errors::ApiError;
use crate::http::torrents::handlers::dispatch_torrent_add_with_catalog;
use crate::http::torrents::labels::{
    load_label_catalog, normalize_label_name, update_label_catalog,
};
use crate::http::torrents::{TorrentHandles, TorrentMetadata, normalize_trackers};
use crate::models::TorrentCreateRequest;

/// Attach qBittorrent-compatible endpoints to the primary router.
pub(crate) fn mount(router: Router<Arc<ApiState>>) -> Router<Arc<ApiState>> {
    router
        .route("/api/v2/auth/login", post(login))
        .route("/api/v2/auth/logout", post(logout))
        .route("/api/v2/app/version", get(app_version))
        .route("/api/v2/app/webapiVersion", get(app_webapi_version))
        .route("/api/v2/sync/maindata", get(sync_maindata))
        .route("/api/v2/sync/torrentPeers", get(sync_torrent_peers))
        .route("/api/v2/torrents/info", get(torrents_info))
        .route("/api/v2/torrents/properties", get(torrents_properties))
        .route("/api/v2/torrents/trackers", get(torrents_trackers))
        .route("/api/v2/torrents/categories", get(list_categories))
        .route("/api/v2/torrents/createCategory", post(create_category))
        .route("/api/v2/torrents/tags", get(list_tags))
        .route("/api/v2/torrents/createTags", post(create_tags))
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
    username: Option<String>,
    password: Option<String>,
}

async fn login(
    State(state): State<Arc<ApiState>>,
    Form(form): Form<LoginForm>,
) -> Result<Response, ApiError> {
    let sid = state.issue_qb_session();
    if form.username.is_some() || form.password.is_some() {
        warn!("ignored qbittorrent login credentials (compatibility mode)");
    }
    let mut response = ok_plain();
    response
        .headers_mut()
        .insert(SET_COOKIE, session_cookie_header(&sid)?);
    Ok(response)
}

async fn logout(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let sid = ensure_session(&state, &headers)?;
    state.revoke_qb_session(&sid);
    let mut response = ok_plain();
    response
        .headers_mut()
        .insert(SET_COOKIE, clear_session_cookie());
    Ok(response)
}

async fn app_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    ensure_session(&state, &headers)?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    Ok((
        response_headers,
        format!("Revaer {}", env!("CARGO_PKG_VERSION")),
    ))
}

async fn app_webapi_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    ensure_session(&state, &headers)?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    Ok((response_headers, "2.8.18"))
}

#[derive(Deserialize, Default)]
pub(crate) struct SyncParams {
    pub rid: Option<u64>,
}

#[derive(Deserialize)]
pub(crate) struct TorrentHashQuery {
    pub hash: String,
}

#[derive(Deserialize)]
pub(crate) struct TorrentHashWithRidQuery {
    pub hash: String,
    #[serde(default)]
    pub rid: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct SyncMainData {
    pub full_update: bool,
    pub rid: u64,
    pub torrents: HashMap<String, QbTorrentEntry>,
    #[serde(default)]
    pub torrents_removed: Vec<String>,
    pub categories: HashMap<String, QbCategory>,
    pub server_state: QbServerState,
}

#[derive(Serialize, Default)]
pub(crate) struct QbCategory {
    pub name: String,
    pub save_path: String,
}

#[derive(Serialize)]
pub(crate) struct QbServerState {
    pub dl_info_speed: i64,
    pub up_info_speed: i64,
    pub dl_rate_limit: i64,
    pub up_rate_limit: i64,
    pub dl_info_data: i64,
    pub up_info_data: i64,
    pub queueing: bool,
}

#[derive(Serialize)]
pub(crate) struct QbTorrentEntry {
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

#[derive(Serialize, Default)]
pub(crate) struct QbTorrentProperties {
    pub save_path: String,
    pub name: String,
    pub hash: String,
    pub creation_date: i64,
    pub completion_on: i64,
    pub added_on: i64,
    pub last_activity: i64,
    pub total_size: i64,
    pub progress: f64,
    pub ratio: f64,
    pub dl_limit: i64,
    pub up_limit: i64,
    pub time_active: i64,
    pub seeding_time: i64,
    pub eta: i64,
    pub super_seeding: bool,
    pub force_start: bool,
    pub private: bool,
    pub comment: String,
    pub tags: String,
    pub category: String,
}

#[derive(Serialize)]
pub(crate) struct QbTrackerEntry {
    pub url: String,
    pub status: i32,
    pub num_peers: i32,
    pub msg: String,
    pub tier: i32,
}

#[derive(Serialize, Default)]
pub(crate) struct QbPeerList {
    pub rid: u64,
    pub peers: HashMap<String, Value>,
    #[serde(default)]
    pub peers_removed: Vec<String>,
    pub full_update: bool,
}

pub(crate) async fn sync_maindata(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(params): Query<SyncParams>,
) -> Result<Json<SyncMainData>, ApiError> {
    ensure_session(&state, &headers)?;
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let status_map: HashMap<Uuid, TorrentStatus> = handles
        .inspector()
        .list()
        .await
        .map_err(|err| {
            error!(error = %err, "failed to list torrents for qB sync");
            ApiError::internal("failed to list torrents")
        })?
        .into_iter()
        .map(|status| (status.id, status))
        .collect();

    let requested_rid = params.rid.unwrap_or(0);
    let last_event_id = state.events.last_event_id().unwrap_or(requested_rid);

    let events_since = if requested_rid == 0 {
        Vec::new()
    } else {
        state.events.backlog_since(requested_rid)
    };

    let mut changed = HashSet::new();
    for envelope in &events_since {
        match &envelope.event {
            CoreEvent::TorrentAdded { torrent_id, .. }
            | CoreEvent::FilesDiscovered { torrent_id, .. }
            | CoreEvent::Progress { torrent_id, .. }
            | CoreEvent::StateChanged { torrent_id, .. }
            | CoreEvent::Completed { torrent_id, .. }
            | CoreEvent::TorrentRemoved { torrent_id }
            | CoreEvent::FsopsStarted { torrent_id }
            | CoreEvent::FsopsProgress { torrent_id, .. }
            | CoreEvent::FsopsCompleted { torrent_id }
            | CoreEvent::FsopsFailed { torrent_id, .. } => {
                changed.insert(*torrent_id);
            }
            _ => {}
        }
    }

    let buffer_gap = requested_rid != 0 && last_event_id > requested_rid && events_since.is_empty();
    let need_full_update = requested_rid == 0 || buffer_gap;

    let mut torrents = HashMap::new();
    let mut torrents_removed = Vec::new();

    if need_full_update {
        for status in status_map.values() {
            let metadata = state.get_metadata(&status.id);
            let entry = qb_entry(status, &metadata);
            torrents.insert(entry.hash.clone(), entry);
        }
    } else {
        for torrent_id in &changed {
            if let Some(status) = status_map.get(torrent_id) {
                let metadata = state.get_metadata(torrent_id);
                let entry = qb_entry(status, &metadata);
                torrents.insert(entry.hash.clone(), entry);
            } else {
                torrents_removed.push(torrent_id.simple().to_string());
            }
        }
    }

    let server_state = build_server_state(status_map.values().map(|status| {
        (
            i64::try_from(status.rates.download_bps).unwrap_or(i64::MAX),
            i64::try_from(status.rates.upload_bps).unwrap_or(i64::MAX),
            i64::try_from(status.progress.bytes_downloaded).unwrap_or(i64::MAX),
            0,
        )
    }));

    Ok(Json(SyncMainData {
        full_update: need_full_update,
        rid: last_event_id,
        torrents,
        torrents_removed,
        categories: HashMap::new(),
        server_state,
    }))
}

pub(crate) async fn sync_torrent_peers(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(params): Query<TorrentHashWithRidQuery>,
) -> Result<Json<QbPeerList>, ApiError> {
    ensure_session(&state, &headers)?;
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    resolve_hashes(handles, &params.hash).await?;
    let rid = params
        .rid
        .unwrap_or_else(|| state.events.last_event_id().unwrap_or(0));
    Ok(Json(QbPeerList {
        rid,
        peers: HashMap::new(),
        peers_removed: Vec::new(),
        full_update: true,
    }))
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct TorrentsInfoParams {
    pub hashes: Option<String>,
}

pub(crate) async fn torrents_info(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(params): Query<TorrentsInfoParams>,
) -> Result<Json<Vec<QbTorrentEntry>>, ApiError> {
    ensure_session(&state, &headers)?;
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

pub(crate) async fn torrents_properties(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(params): Query<TorrentHashQuery>,
) -> Result<Json<QbTorrentProperties>, ApiError> {
    ensure_session(&state, &headers)?;
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let ids = resolve_hashes(handles, &params.hash).await?;
    let id = *ids
        .first()
        .ok_or_else(|| ApiError::bad_request("hash required"))?;

    let status = handles
        .inspector()
        .get(id)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to fetch torrent status");
            ApiError::internal("failed to fetch torrent status")
        })?
        .ok_or_else(|| ApiError::not_found("torrent not found"))?;
    let metadata = state.get_metadata(&id);
    let now = Utc::now().timestamp();

    let properties = QbTorrentProperties {
        save_path: status
            .download_dir
            .clone()
            .or_else(|| status.library_path.clone())
            .unwrap_or_default(),
        name: status
            .name
            .clone()
            .unwrap_or_else(|| id.simple().to_string()),
        hash: id.simple().to_string(),
        creation_date: status.added_at.timestamp(),
        completion_on: status.completed_at.map_or(0, |dt| dt.timestamp()),
        added_on: status.added_at.timestamp(),
        last_activity: status.last_updated.timestamp(),
        total_size: i64::try_from(status.progress.bytes_total).unwrap_or(i64::MAX),
        progress: progress_fraction(&status),
        ratio: status.rates.ratio,
        dl_limit: metadata
            .rate_limit
            .as_ref()
            .and_then(|limit| {
                limit
                    .download_bps
                    .and_then(|value| i64::try_from(value).ok())
            })
            .unwrap_or(-1),
        up_limit: metadata
            .rate_limit
            .as_ref()
            .and_then(|limit| limit.upload_bps.and_then(|value| i64::try_from(value).ok()))
            .unwrap_or(-1),
        time_active: now.saturating_sub(status.added_at.timestamp()),
        seeding_time: status
            .completed_at
            .map_or(0, |ts| now.saturating_sub(ts.timestamp())),
        eta: status
            .progress
            .eta_seconds
            .map_or(-1, |eta| i64::try_from(eta).unwrap_or(-1)),
        super_seeding: metadata.super_seeding.unwrap_or(false),
        force_start: false,
        private: status.private.unwrap_or(false),
        comment: status.comment.clone().unwrap_or_default(),
        tags: metadata.tags.join(","),
        category: metadata.category.clone().unwrap_or_default(),
    };

    Ok(Json(properties))
}

pub(crate) async fn torrents_trackers(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(params): Query<TorrentHashQuery>,
) -> Result<Json<Vec<QbTrackerEntry>>, ApiError> {
    ensure_session(&state, &headers)?;
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let ids = resolve_hashes(handles, &params.hash).await?;
    let id = *ids
        .first()
        .ok_or_else(|| ApiError::bad_request("hash required"))?;
    let metadata = state.get_metadata(&id);

    let mut trackers = Vec::new();
    for (idx, url) in metadata.trackers.iter().enumerate() {
        let message = metadata
            .tracker_messages
            .get(url)
            .cloned()
            .unwrap_or_default();
        trackers.push(QbTrackerEntry {
            url: url.clone(),
            status: 2,
            num_peers: 0,
            msg: message,
            tier: i32::try_from(idx).unwrap_or_default(),
        });
    }

    Ok(Json(trackers))
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct TorrentAddForm {
    pub urls: Option<String>,
    pub tags: Option<String>,
    #[serde(rename = "savepath")]
    pub save_path: Option<String>,
    #[serde(rename = "sequentialDownload")]
    pub sequential: Option<bool>,
}

pub(crate) async fn torrents_add(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TorrentAddForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    let Some(urls) = form
        .urls
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return Err(ApiError::bad_request("urls parameter is required"));
    };

    let tags = parse_tags(form.tags.as_deref());
    let catalog = if tags.is_empty() {
        None
    } else {
        Some(load_label_catalog(state.as_ref()).await?)
    };
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

        let trackers = normalize_trackers(&request.trackers)?;
        let add_request = dispatch_torrent_add_with_catalog(
            state.as_ref(),
            &request,
            trackers,
            Vec::new(),
            catalog.as_ref(),
        )
        .await?;
        state.set_metadata(
            request.id,
            TorrentMetadata::from_options(&add_request.options),
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
pub(crate) struct TorrentHashesForm {
    pub hashes: String,
}

pub(crate) async fn torrents_pause(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TorrentHashesForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    apply_to_hashes(&state, &form.hashes, |workflow, id| async move {
        workflow.pause_torrent(id).await
    })
    .await?;
    Ok(ok_plain())
}

pub(crate) async fn torrents_resume(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TorrentHashesForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    apply_to_hashes(&state, &form.hashes, |workflow, id| async move {
        workflow.resume_torrent(id).await
    })
    .await?;
    Ok(ok_plain())
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct TorrentDeleteForm {
    pub hashes: String,
    #[serde(rename = "deleteFiles")]
    pub delete_files: Option<bool>,
}

pub(crate) async fn torrents_delete(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TorrentDeleteForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
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

pub(crate) async fn list_categories(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<HashMap<String, QbCategory>>, ApiError> {
    ensure_session(&state, &headers)?;
    let catalog = load_label_catalog(state.as_ref()).await?;
    let mut categories = HashMap::new();
    for (name, policy) in catalog.categories {
        let save_path = policy.download_dir.unwrap_or_default();
        categories.insert(name.clone(), QbCategory { name, save_path });
    }
    Ok(Json(categories))
}

#[derive(Deserialize)]
pub(crate) struct CreateCategoryForm {
    #[serde(rename = "category")]
    category: Option<String>,
    #[serde(rename = "savePath")]
    save_path: Option<String>,
}

pub(crate) async fn create_category(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<CreateCategoryForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    let Some(name) = form.category.as_deref() else {
        return Err(ApiError::bad_request("category is required"));
    };
    let normalized = normalize_label_name("category", name)?;
    let download_dir = form
        .save_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    update_label_catalog(
        state.as_ref(),
        "compat_qb",
        "torrent_category_upsert",
        |catalog| {
            catalog.upsert_category(
                &normalized,
                TorrentLabelPolicy {
                    download_dir,
                    ..TorrentLabelPolicy::default()
                },
            )
        },
    )
    .await?;
    Ok(ok_plain())
}

pub(crate) async fn list_tags(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, ApiError> {
    ensure_session(&state, &headers)?;
    let catalog = load_label_catalog(state.as_ref()).await?;
    let mut tags: Vec<String> = catalog.tags.into_keys().collect();
    tags.sort();
    Ok(Json(tags))
}

#[derive(Deserialize)]
pub(crate) struct CreateTagsForm {
    #[serde(rename = "tags")]
    tags: Option<String>,
}

pub(crate) async fn create_tags(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<CreateTagsForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    let tags = parse_tags(form.tags.as_deref());
    if tags.is_empty() {
        return Err(ApiError::bad_request("tags are required"));
    }
    update_label_catalog(
        state.as_ref(),
        "compat_qb",
        "torrent_tag_upsert",
        |catalog| {
            for tag in tags {
                let normalized = normalize_label_name("tag", &tag)?;
                catalog.upsert_tag(&normalized, TorrentLabelPolicy::default())?;
            }
            Ok(())
        },
    )
    .await?;
    Ok(ok_plain())
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct TransferLimitForm {
    pub limit: String,
}

pub(crate) async fn transfer_upload_limit(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TransferLimitForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
    apply_rate_limit(&state, None, parse_limit(&form.limit)?).await?;
    Ok(ok_plain())
}

pub(crate) async fn transfer_download_limit(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Form(form): Form<TransferLimitForm>,
) -> Result<Response, ApiError> {
    ensure_session(&state, &headers)?;
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
        category: metadata.category.clone().unwrap_or_default(),
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

fn progress_fraction(status: &TorrentStatus) -> f64 {
    if status.progress.bytes_total == 0 {
        0.0
    } else {
        bytes_to_f64(status.progress.bytes_downloaded) / bytes_to_f64(status.progress.bytes_total)
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

fn ensure_session(state: &Arc<ApiState>, headers: &HeaderMap) -> Result<String, ApiError> {
    let sid = sid_from_headers(headers).ok_or_else(|| {
        ApiError::forbidden("session cookie missing; authenticate via /api/v2/auth/login")
    })?;
    if state.validate_qb_session(&sid) {
        Ok(sid)
    } else {
        Err(ApiError::forbidden(
            "session expired; authenticate via /api/v2/auth/login",
        ))
    }
}

const fn bytes_to_f64(value: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 to f64 conversion is required for qBittorrent compatibility fields"
    )]
    {
        value as f64
    }
}

fn sid_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|raw| cookie_value(raw, "sid"))
}

fn session_cookie_header(token: &str) -> Result<HeaderValue, ApiError> {
    let cookie = format!(
        "SID={token}; HttpOnly; Path=/; Max-Age={}; SameSite=Lax",
        COMPAT_SESSION_TTL.as_secs()
    );
    HeaderValue::from_str(&cookie)
        .map_err(|_| ApiError::internal("failed to encode session cookie header"))
}

const fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_static("SID=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

fn cookie_value(raw: &str, needle: &str) -> Option<String> {
    for entry in raw.split(';') {
        let mut parts = entry.splitn(2, '=');
        let name = parts.next().map(str::trim).unwrap_or_default();
        if !name.eq_ignore_ascii_case(needle) {
            continue;
        }
        if let Some(value) = parts.next().map(str::trim) {
            if value.is_empty() {
                return None;
            }
            return Some(value.trim_matches('"').to_string());
        }
        return None;
    }
    None
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

pub(crate) fn parse_limit(raw: &str) -> Result<Option<u64>, ApiError> {
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
        Ok(u64::try_from(value).ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFacade;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use axum::{
        extract::{Form, State},
        http::{
            HeaderMap, HeaderValue, StatusCode,
            header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
        },
        response::IntoResponse,
    };
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigSnapshot, SettingsChangeset,
        SetupToken,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use revaer_torrent_core::{
        AddTorrent, PeerSnapshot, TorrentInspector, TorrentProgress, TorrentRates, TorrentStatus,
        TorrentWorkflow,
    };
    use serde_json::Value;
    use std::{sync::Arc, time::Duration};

    #[tokio::test]
    async fn login_emits_cookie_and_accepts_credentials() -> Result<()> {
        let state = test_state();
        let response = login(State(Arc::clone(&state)), Form(LoginForm::default()))
            .await
            .expect("login should succeed");
        let headers = response.headers().clone();
        let cookie = headers
            .get(SET_COOKIE)
            .expect("session cookie present")
            .to_str()?;
        let session = cookie_value(cookie, "sid").expect("sid present in cookie");
        assert_eq!(
            headers
                .get(CONTENT_TYPE)
                .expect("content type present")
                .to_str()?,
            "text/plain; charset=utf-8"
        );
        assert!(state.validate_qb_session(&session));

        login(
            State(state),
            Form(LoginForm {
                username: Some("demo".into()),
                password: Some("secret".into()),
            }),
        )
        .await
        .expect("login with credentials should still succeed");
        Ok(())
    }

    #[tokio::test]
    async fn logout_revokes_session_and_clears_cookie() -> Result<()> {
        let state = test_state();
        let response = login(State(Arc::clone(&state)), Form(LoginForm::default()))
            .await
            .expect("login succeeds");
        let cookie = response
            .headers()
            .get(SET_COOKIE)
            .expect("cookie present")
            .to_str()?;
        let sid = cookie_value(cookie, "sid").expect("sid present");
        assert!(state.validate_qb_session(&sid));

        let headers = header_with_sid(&sid);
        let response = logout(State(Arc::clone(&state)), headers)
            .await
            .expect("logout succeeds");
        assert_eq!(
            response
                .headers()
                .get(SET_COOKIE)
                .expect("logout cookie present")
                .to_str()?,
            "SID=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax"
        );
        assert!(!state.validate_qb_session(&sid));
        Ok(())
    }

    #[tokio::test]
    async fn version_endpoints_require_session() {
        let state = test_state();
        let headers = HeaderMap::new();
        let result = app_version(State(Arc::clone(&state)), headers).await;
        let error = match result {
            Ok(response) => {
                let response = response.into_response();
                panic!(
                    "expected auth failure but received status {}",
                    response.status()
                );
            }
            Err(err) => err,
        };
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn version_endpoints_emit_plain_text_when_authenticated() -> Result<()> {
        let state = test_state();
        let sid = state.issue_qb_session();
        let headers = header_with_sid(&sid);
        let response = app_version(State(Arc::clone(&state)), headers)
            .await
            .expect("version request succeeds")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .expect("content type present")
                .to_str()?,
            "text/plain; charset=utf-8"
        );

        let headers = header_with_sid(&state.issue_qb_session());
        let response = app_webapi_version(State(state), headers)
            .await
            .expect("webapi version succeeds")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .expect("content type present")
                .to_str()?,
            "text/plain; charset=utf-8"
        );
        Ok(())
    }

    #[tokio::test]
    async fn torrent_properties_returns_snapshot() -> Result<()> {
        let progress = TorrentProgress {
            bytes_downloaded: 512,
            bytes_total: 1024,
            eta_seconds: Some(30),
        };
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("demo".into()),
            download_dir: Some("/downloads".into()),
            progress,
            rates: TorrentRates {
                download_bps: 128_000,
                upload_bps: 64_000,
                ratio: 1.25,
            },
            ..TorrentStatus::default()
        };

        let metadata = TorrentMetadata {
            tags: vec!["alpha".into(), "beta".into()],
            rate_limit: Some(TorrentRateLimit {
                download_bps: Some(1_000),
                upload_bps: Some(2_000),
            }),
            ..TorrentMetadata::default()
        };

        let state = state_with_handles(vec![status.clone()], vec![(status.id, metadata)]);
        let sid = state.issue_qb_session();
        let headers = header_with_sid(&sid);
        let Json(body) = torrents_properties(
            State(state),
            headers,
            Query(TorrentHashQuery {
                hash: status.id.simple().to_string(),
            }),
        )
        .await
        .expect("properties should resolve");

        assert_eq!(body.hash, status.id.simple().to_string());
        assert_eq!(body.name, "demo");
        assert_eq!(body.save_path, "/downloads");
        assert_eq!(body.tags, "alpha,beta");
        assert!(body.progress > 0.0);
        Ok(())
    }

    #[tokio::test]
    async fn torrents_trackers_emit_metadata() -> Result<()> {
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            ..TorrentStatus::default()
        };
        let metadata = TorrentMetadata {
            trackers: vec!["udp://tracker.test/announce".into()],
            ..TorrentMetadata::default()
        };

        let state = state_with_handles(vec![status.clone()], vec![(status.id, metadata)]);
        let sid = state.issue_qb_session();
        let headers = header_with_sid(&sid);
        let Json(entries) = torrents_trackers(
            State(state),
            headers,
            Query(TorrentHashQuery {
                hash: status.id.simple().to_string(),
            }),
        )
        .await
        .expect("trackers should resolve");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "udp://tracker.test/announce");
        Ok(())
    }

    #[tokio::test]
    async fn torrent_peers_returns_empty_list() -> Result<()> {
        let state = state_with_handles(Vec::new(), Vec::new());
        let sid = state.issue_qb_session();
        let headers = header_with_sid(&sid);
        let Json(peers) = sync_torrent_peers(
            State(state),
            headers,
            Query(TorrentHashWithRidQuery {
                hash: Uuid::new_v4().simple().to_string(),
                rid: Some(42),
            }),
        )
        .await
        .expect("peers should resolve");

        assert_eq!(peers.rid, 42);
        assert!(peers.peers.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn categories_and_tags_endpoints_round_trip() -> Result<()> {
        let state = test_state();
        let sid = state.issue_qb_session();
        let headers = header_with_sid(&sid);

        let Json(categories) = list_categories(State(Arc::clone(&state)), headers.clone())
            .await
            .expect("categories should resolve");
        assert!(categories.is_empty());

        let Json(tags) = list_tags(State(Arc::clone(&state)), headers.clone())
            .await
            .expect("tags should resolve");
        assert!(tags.is_empty());

        create_category(
            State(Arc::clone(&state)),
            headers.clone(),
            Form(CreateCategoryForm {
                category: Some("movies".into()),
                save_path: Some("/downloads".into()),
            }),
        )
        .await
        .expect("create category");

        create_tags(
            State(Arc::clone(&state)),
            headers,
            Form(CreateTagsForm {
                tags: Some("alpha,beta".into()),
            }),
        )
        .await
        .expect("create tags");

        let Json(categories) = list_categories(State(Arc::clone(&state)), header_with_sid(&sid))
            .await
            .expect("categories should resolve");
        assert_eq!(categories.len(), 1);
        let entry = categories.get("movies").expect("category entry");
        assert_eq!(entry.save_path, "/downloads");

        let Json(tags) = list_tags(State(state), header_with_sid(&sid))
            .await
            .expect("tags should resolve");
        assert_eq!(tags, vec!["alpha".to_string(), "beta".to_string()]);

        Ok(())
    }

    fn header_with_sid(sid: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("SID={sid}")).expect("valid cookie header"),
        );
        headers
    }

    fn test_state() -> Arc<ApiState> {
        let config: Arc<dyn ConfigFacade> = Arc::new(TestConfig::default());
        Arc::new(ApiState::new(
            config,
            Metrics::new().expect("metrics init"),
            Arc::new(Value::Null),
            EventBus::with_capacity(4),
            None,
        ))
    }

    fn state_with_handles(
        statuses: Vec<TorrentStatus>,
        metadata: Vec<(Uuid, TorrentMetadata)>,
    ) -> Arc<ApiState> {
        let config: Arc<dyn ConfigFacade> = Arc::new(TestConfig::default());
        let handles =
            TorrentHandles::new(Arc::new(StubWorkflow), Arc::new(StubInspector { statuses }));
        let state = Arc::new(ApiState::new(
            config,
            Metrics::new().expect("metrics init"),
            Arc::new(Value::Null),
            EventBus::with_capacity(4),
            Some(handles),
        ));
        for (id, data) in metadata {
            state.set_metadata(id, data);
        }
        state
    }

    #[derive(Clone, Default)]
    struct TestConfig {
        features: Arc<tokio::sync::Mutex<Value>>,
    }

    #[async_trait]
    impl ConfigFacade for TestConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "compat".to_string(),
                mode: AppMode::Active,
                version: 1,
                http_port: 8080,
                bind_addr: std::net::IpAddr::from([127, 0, 0, 1]),
                telemetry: serde_json::json!({}),
                features: self.features.lock().await.clone(),
                immutable_keys: serde_json::json!([]),
            })
        }

        async fn issue_setup_token(&self, _ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            Err(anyhow!("not implemented in tests"))
        }

        async fn validate_setup_token(&self, _token: &str) -> Result<()> {
            Err(anyhow!("not implemented in tests"))
        }

        async fn consume_setup_token(&self, _token: &str) -> Result<()> {
            Err(anyhow!("not implemented in tests"))
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            if let Some(app_profile) = changeset.app_profile.as_ref()
                && let Some(features) = app_profile.get("features")
            {
                *self.features.lock().await = features.clone();
            }
            Ok(AppliedChanges {
                revision: 1,
                app_profile: Some(self.get_app_profile().await?),
                engine_profile: None,
                fs_policy: None,
            })
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            Err(anyhow!("not implemented in tests"))
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            Ok(None)
        }
    }

    #[derive(Clone)]
    struct StubWorkflow;

    #[async_trait]
    impl TorrentWorkflow for StubWorkflow {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct StubInspector {
        statuses: Vec<TorrentStatus>,
    }

    #[async_trait]
    impl TorrentInspector for StubInspector {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(self.statuses.clone())
        }

        async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(self.statuses.iter().find(|status| status.id == id).cloned())
        }

        async fn peers(&self, _id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }
    }
}
