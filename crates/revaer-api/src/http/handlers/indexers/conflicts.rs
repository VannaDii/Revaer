//! Source metadata conflict endpoints for indexer operators.
//!
//! # Design
//! - Expose unresolved and resolved source metadata conflicts for operator review.
//! - Delegate resolution and reopen actions to the injected indexer facade.
//! - Surface stable RFC9457 errors with structured context fields.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::Deserialize;

use crate::app::indexers::{
    SourceMetadataConflictServiceError, SourceMetadataConflictServiceErrorKind,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::trim_and_filter_empty;
use crate::models::{
    IndexerSourceMetadataConflictListResponse, IndexerSourceMetadataConflictReopenRequest,
    IndexerSourceMetadataConflictResolveRequest,
};

const CONFLICT_LIST_FAILED: &str = "failed to list source metadata conflicts";
const CONFLICT_RESOLVE_FAILED: &str = "failed to resolve source metadata conflict";
const CONFLICT_REOPEN_FAILED: &str = "failed to reopen source metadata conflict";

#[derive(Debug, Deserialize)]
pub(crate) struct ConflictListQuery {
    include_resolved: Option<bool>,
    limit: Option<i32>,
}

pub(crate) async fn list_source_metadata_conflicts(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ConflictListQuery>,
) -> Result<Json<IndexerSourceMetadataConflictListResponse>, ApiError> {
    let conflicts = state
        .indexers
        .source_metadata_conflict_list(SYSTEM_ACTOR_PUBLIC_ID, query.include_resolved, query.limit)
        .await
        .map_err(|err| {
            map_source_metadata_conflict_error(
                "source_metadata_conflict_list",
                CONFLICT_LIST_FAILED,
                &err,
            )
        })?;

    Ok(Json(IndexerSourceMetadataConflictListResponse {
        conflicts,
    }))
}

pub(crate) async fn resolve_source_metadata_conflict(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerSourceMetadataConflictResolveRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .source_metadata_conflict_resolve(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.conflict_id,
            request.resolution.trim(),
            trim_and_filter_empty(request.resolution_note.as_deref()),
        )
        .await
        .map_err(|err| {
            map_source_metadata_conflict_error(
                "source_metadata_conflict_resolve",
                CONFLICT_RESOLVE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn reopen_source_metadata_conflict(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerSourceMetadataConflictReopenRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .source_metadata_conflict_reopen(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.conflict_id,
            trim_and_filter_empty(request.resolution_note.as_deref()),
        )
        .await
        .map_err(|err| {
            map_source_metadata_conflict_error(
                "source_metadata_conflict_reopen",
                CONFLICT_REOPEN_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_source_metadata_conflict_error(
    operation: &'static str,
    detail: &'static str,
    err: &SourceMetadataConflictServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        SourceMetadataConflictServiceErrorKind::Invalid => ApiError::bad_request(detail),
        SourceMetadataConflictServiceErrorKind::NotFound => ApiError::not_found(detail),
        SourceMetadataConflictServiceErrorKind::Conflict => ApiError::conflict(detail),
        SourceMetadataConflictServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        SourceMetadataConflictServiceErrorKind::Storage => ApiError::internal(detail),
    };

    api_error = api_error.with_context_field("operation", operation);
    if let Some(code) = err.code() {
        api_error = api_error.with_context_field("error_code", code);
    }
    if let Some(sqlstate) = err.sqlstate() {
        api_error = api_error.with_context_field("sqlstate", sqlstate);
    }
    api_error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::handlers::indexers::test_support::{RecordingIndexers, indexer_test_state};
    use axum::{Json, extract::State};
    use std::sync::Arc;

    #[tokio::test]
    async fn resolve_source_metadata_conflict_filters_blank_note() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let request = IndexerSourceMetadataConflictResolveRequest {
            conflict_id: 42,
            resolution: " accepted_incoming ".to_string(),
            resolution_note: Some("   ".to_string()),
        };

        let status = resolve_source_metadata_conflict(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .source_metadata_conflict_resolve_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, 42);
        assert_eq!(calls[0].2, "accepted_incoming");
        assert_eq!(calls[0].3, None);
        Ok(())
    }

    #[tokio::test]
    async fn reopen_source_metadata_conflict_filters_blank_note() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let request = IndexerSourceMetadataConflictReopenRequest {
            conflict_id: 7,
            resolution_note: Some("   ".to_string()),
        };

        let status = reopen_source_metadata_conflict(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .source_metadata_conflict_reopen_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, 7);
        assert_eq!(calls[0].2, None);
        Ok(())
    }
}
