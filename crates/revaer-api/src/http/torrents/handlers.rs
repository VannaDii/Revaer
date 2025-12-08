//! Torrent route handlers and workflow integration.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path as AxumPath, Query, State},
    http::StatusCode,
};
use base64::{Engine as _, engine::general_purpose};
use tracing::{error, info};
use uuid::Uuid;

use crate::app::state::ApiState;
use crate::http::constants::{DEFAULT_PAGE_SIZE, MAX_METAINFO_BYTES, MAX_PAGE_SIZE};
use crate::http::errors::ApiError;
use crate::models::{
    TorrentAction, TorrentCreateRequest, TorrentDetail, TorrentListResponse,
    TorrentSelectionRequest, TorrentStateKind, TorrentSummary,
};
use revaer_events::Event as CoreEvent;
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit, TorrentSource, TorrentStatus,
};

use super::{
    CursorToken, StatusEntry, TorrentHandles, TorrentListQuery, TorrentMetadata,
    decode_cursor_token, detail_from_components, encode_cursor_from_entry, normalise_lower,
    parse_state_filter, split_comma_separated, summary_from_components,
};

pub(crate) async fn create_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<crate::http::auth::AuthContext>,
    Json(request): Json<TorrentCreateRequest>,
) -> Result<StatusCode, ApiError> {
    match context {
        crate::http::auth::AuthContext::ApiKey { .. } => {}
        crate::http::auth::AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    dispatch_torrent_add(state.torrent.as_ref(), &request).await?;
    state.set_metadata(
        request.id,
        TorrentMetadata::new(request.tags.clone(), request.trackers.clone()),
    );
    let torrent_name = request.name.as_deref().unwrap_or("<unspecified>");
    info!(torrent_id = %request.id, torrent_name = %torrent_name, "torrent submission requested");
    state.update_torrent_metrics().await;

    Ok(StatusCode::ACCEPTED)
}

pub(crate) async fn delete_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<crate::http::auth::AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    match context {
        crate::http::auth::AuthContext::ApiKey { .. } => {}
        crate::http::auth::AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    dispatch_torrent_remove(state.torrent.as_ref(), id).await?;
    info!(torrent_id = %id, "torrent removal requested");
    state.remove_metadata(&id);
    state.update_torrent_metrics().await;
    let _ = state
        .events
        .publish(CoreEvent::TorrentRemoved { torrent_id: id });
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn select_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<crate::http::auth::AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
    Json(request): Json<TorrentSelectionRequest>,
) -> Result<StatusCode, ApiError> {
    match context {
        crate::http::auth::AuthContext::ApiKey { .. } => {}
        crate::http::auth::AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let update: FileSelectionUpdate = request.into();
    handles
        .workflow()
        .update_selection(id, update)
        .await
        .map_err(|err| {
            error!(error = %err, torrent_id = %id, "failed to update torrent selection");
            ApiError::internal("failed to update torrent selection")
        })?;
    info!(torrent_id = %id, "torrent selection update requested");
    Ok(StatusCode::ACCEPTED)
}

pub(crate) async fn action_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<crate::http::auth::AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
    Json(action): Json<TorrentAction>,
) -> Result<StatusCode, ApiError> {
    match context {
        crate::http::auth::AuthContext::ApiKey { .. } => {}
        crate::http::auth::AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let workflow = handles.workflow();

    let result = match &action {
        TorrentAction::Pause => workflow.pause_torrent(id).await,
        TorrentAction::Resume => workflow.resume_torrent(id).await,
        TorrentAction::Remove { delete_data } => {
            let options = RemoveTorrent {
                with_data: *delete_data,
            };
            workflow.remove_torrent(id, options).await
        }
        TorrentAction::Reannounce => workflow.reannounce(id).await,
        TorrentAction::Recheck => workflow.recheck(id).await,
        TorrentAction::Sequential { enable } => workflow.set_sequential(id, *enable).await,
        TorrentAction::Rate {
            download_bps,
            upload_bps,
        } => {
            workflow
                .update_limits(
                    Some(id),
                    TorrentRateLimit {
                        download_bps: *download_bps,
                        upload_bps: *upload_bps,
                    },
                )
                .await
        }
    };

    result.map_err(|err| {
        error!(error = %err, torrent_id = %id, "torrent action failed");
        ApiError::internal("failed to execute torrent action")
    })?;

    if matches!(action, TorrentAction::Remove { .. }) {
        state.remove_metadata(&id);
    }
    info!(torrent_id = %id, action = ?action, "torrent action dispatched");
    Ok(StatusCode::ACCEPTED)
}

pub(crate) async fn fetch_all_torrents(
    handles: &TorrentHandles,
) -> Result<Vec<TorrentStatus>, ApiError> {
    handles.inspector().list().await.map_err(|err| {
        error!(error = %err, "failed to read torrent catalogue");
        ApiError::internal("failed to query torrent status")
    })
}

pub(crate) async fn fetch_torrent_status(
    handles: &TorrentHandles,
    id: Uuid,
) -> Result<TorrentStatus, ApiError> {
    handles
        .inspector()
        .get(id)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to load torrent status");
            ApiError::internal("failed to query torrent status")
        })?
        .ok_or_else(|| ApiError::not_found("torrent not found"))
}

pub(crate) async fn list_torrents(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<TorrentListQuery>,
) -> Result<Json<TorrentListResponse>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let statuses = fetch_all_torrents(handles).await?;
    state.record_torrent_metrics(&statuses);

    let filters = parse_list_filters(&query)?;
    let mut entries = build_status_entries(statuses, &state);
    filter_entries(&mut entries, &filters);
    sort_entries(&mut entries);

    let cursor = query
        .cursor
        .as_ref()
        .map(|token| decode_cursor_token(token))
        .transpose()?;
    let limit = query
        .limit
        .map_or(DEFAULT_PAGE_SIZE, |value| value as usize)
        .clamp(1, MAX_PAGE_SIZE);
    let start_index = compute_start_index(&entries, cursor.as_ref());
    let (torrents, next) = paginate_entries(&entries, start_index, limit)?;

    Ok(Json(TorrentListResponse { torrents, next }))
}

pub(crate) async fn get_torrent(
    State(state): State<Arc<ApiState>>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<TorrentDetail>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let status = fetch_torrent_status(handles, id).await?;
    state.record_torrent_metrics(std::slice::from_ref(&status));
    let metadata = state.get_metadata(&status.id);
    Ok(Json(detail_from_components(status, metadata)))
}

struct ListFilters {
    state: Option<TorrentStateKind>,
    tags: HashSet<String>,
    tracker: Option<String>,
    extension: Option<String>,
    name: Option<String>,
}

fn parse_list_filters(query: &TorrentListQuery) -> Result<ListFilters, ApiError> {
    let state = match query.state.as_deref() {
        Some(filter) => Some(parse_state_filter(filter)?),
        None => None,
    };
    let tags = query
        .tags
        .as_deref()
        .map(split_comma_separated)
        .unwrap_or_default()
        .into_iter()
        .map(|tag| tag.to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<HashSet<_>>();
    let tracker = query.tracker.as_ref().map(|value| normalise_lower(value));
    let extension = query
        .extension
        .as_ref()
        .map(|value| normalise_lower(value.trim_start_matches('.')));
    let name = query.name.as_ref().map(|value| normalise_lower(value));

    Ok(ListFilters {
        state,
        tags,
        tracker,
        extension,
        name,
    })
}

fn build_status_entries(statuses: Vec<TorrentStatus>, state: &Arc<ApiState>) -> Vec<StatusEntry> {
    statuses
        .into_iter()
        .map(|status| StatusEntry {
            metadata: state.get_metadata(&status.id),
            status,
        })
        .collect()
}

fn filter_entries(entries: &mut Vec<StatusEntry>, filters: &ListFilters) {
    entries.retain(|entry| {
        if let Some(expected_state) = filters.state {
            let current = TorrentStateKind::from(entry.status.state.clone());
            if current != expected_state {
                return false;
            }
        }

        if !filters.tags.is_empty() {
            let tags = entry
                .metadata
                .tags
                .iter()
                .map(|tag| tag.to_lowercase())
                .collect::<HashSet<_>>();
            if !filters.tags.iter().all(|needle| tags.contains(needle)) {
                return false;
            }
        }

        if let Some(tracker) = &filters.tracker {
            let matches_tracker = entry
                .metadata
                .trackers
                .iter()
                .any(|value| value.to_lowercase().contains(tracker));
            if !matches_tracker {
                return false;
            }
        }

        if let Some(extension) = &filters.extension {
            let matches_extension = entry.status.files.as_ref().is_some_and(|files| {
                files.iter().any(|file| {
                    file.path
                        .rsplit_once('.')
                        .is_some_and(|(_, ext)| normalise_lower(ext) == *extension)
                })
            });
            if !matches_extension {
                return false;
            }
        }

        if let Some(name) = &filters.name {
            let matched = entry
                .status
                .name
                .as_ref()
                .is_some_and(|value| value.to_lowercase().contains(name));
            if !matched {
                return false;
            }
        }

        true
    });
}

fn sort_entries(entries: &mut [StatusEntry]) {
    entries.sort_by(|a, b| {
        b.status
            .last_updated
            .cmp(&a.status.last_updated)
            .then_with(|| a.status.id.cmp(&b.status.id))
    });
}

fn compute_start_index(entries: &[StatusEntry], cursor: Option<&CursorToken>) -> usize {
    let mut index = 0;
    if let Some(cursor) = cursor {
        while index < entries.len() {
            let status = &entries[index].status;
            if status.last_updated > cursor.last_updated
                || (status.last_updated == cursor.last_updated && status.id >= cursor.id)
            {
                index += 1;
            } else {
                break;
            }
        }
    }
    index
}

fn paginate_entries(
    entries: &[StatusEntry],
    start_index: usize,
    limit: usize,
) -> Result<(Vec<TorrentSummary>, Option<String>), ApiError> {
    let end_index = (start_index + limit).min(entries.len());
    let slice = &entries[start_index..end_index];
    let torrents = slice
        .iter()
        .map(|entry| summary_from_components(entry.status.clone(), entry.metadata.clone()))
        .collect::<Vec<_>>();

    let next = if end_index < entries.len() && !torrents.is_empty() {
        Some(encode_cursor_from_entry(&entries[end_index - 1])?)
    } else {
        None
    };

    Ok((torrents, next))
}

pub(crate) async fn dispatch_torrent_add(
    handles: Option<&TorrentHandles>,
    request: &TorrentCreateRequest,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let add_request = build_add_torrent(request)?;

    handles
        .workflow()
        .add_torrent(add_request)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to add torrent through workflow");
            ApiError::internal("failed to add torrent")
        })
}

pub(crate) async fn dispatch_torrent_remove(
    handles: Option<&TorrentHandles>,
    id: Uuid,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    handles
        .workflow()
        .remove_torrent(id, RemoveTorrent::default())
        .await
        .map_err(|err| {
            error!(error = %err, "failed to remove torrent through workflow");
            ApiError::internal("failed to remove torrent")
        })
}

pub(crate) fn build_add_torrent(request: &TorrentCreateRequest) -> Result<AddTorrent, ApiError> {
    let magnet = request
        .magnet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let source = if let Some(magnet) = magnet {
        TorrentSource::magnet(magnet.to_string())
    } else if let Some(encoded) = &request.metainfo {
        let bytes = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| ApiError::bad_request("metainfo payload must be base64 encoded"))?;
        if bytes.len() > MAX_METAINFO_BYTES {
            return Err(ApiError::bad_request(
                "metainfo payload exceeds the 5 MiB limit",
            ));
        }
        TorrentSource::metainfo(bytes)
    } else {
        return Err(ApiError::bad_request(
            "either magnet or metainfo payload must be provided",
        ));
    };

    let options = request.to_options();

    Ok(AddTorrent {
        id: request.id,
        source,
        options,
    })
}
