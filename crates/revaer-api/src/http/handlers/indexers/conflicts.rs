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
    use crate::app::indexers::{
        SourceMetadataConflictServiceError, SourceMetadataConflictServiceErrorKind,
    };
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::IndexerSourceMetadataConflictResponse;
    use axum::{
        Json,
        extract::{Query, State},
        response::IntoResponse,
    };
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn list_source_metadata_conflicts_returns_rows_and_forwards_query() -> Result<(), ApiError>
    {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers.set_source_metadata_conflict_list_response(vec![
            IndexerSourceMetadataConflictResponse {
                conflict_id: 42,
                conflict_type: "display_name".to_string(),
                existing_value: "old".to_string(),
                incoming_value: "new".to_string(),
                observed_at: Utc
                    .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
                    .single()
                    .expect("valid timestamp"),
                resolved_at: None,
                resolution: None,
                resolution_note: None,
            },
        ]);
        let state = indexer_test_state(indexers.clone())?;

        let response = list_source_metadata_conflicts(
            State(state),
            Query(ConflictListQuery {
                include_resolved: Some(true),
                limit: Some(25),
            }),
        )
        .await?;

        assert_eq!(response.conflicts.len(), 1);
        assert_eq!(response.conflicts[0].conflict_id, 42);
        assert_eq!(response.conflicts[0].conflict_type, "display_name");

        let calls = indexers
            .source_metadata_conflict_list_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls, vec![(SYSTEM_ACTOR_PUBLIC_ID, Some(true), Some(25))]);
        Ok(())
    }

    #[test]
    fn map_source_metadata_conflict_error_maps_status_by_kind() {
        let cases = [
            (
                SourceMetadataConflictServiceErrorKind::Invalid,
                StatusCode::BAD_REQUEST,
            ),
            (
                SourceMetadataConflictServiceErrorKind::NotFound,
                StatusCode::NOT_FOUND,
            ),
            (
                SourceMetadataConflictServiceErrorKind::Conflict,
                StatusCode::CONFLICT,
            ),
            (
                SourceMetadataConflictServiceErrorKind::Unauthorized,
                StatusCode::UNAUTHORIZED,
            ),
            (
                SourceMetadataConflictServiceErrorKind::Storage,
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (kind, expected_status) in cases {
            let response = map_source_metadata_conflict_error(
                "source_metadata_conflict_list",
                CONFLICT_LIST_FAILED,
                &SourceMetadataConflictServiceError::new(kind),
            )
            .into_response();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[tokio::test]
    async fn resolve_source_metadata_conflict_maps_error_context_fields() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers.set_source_metadata_conflict_error(
            SourceMetadataConflictServiceError::new(
                SourceMetadataConflictServiceErrorKind::Conflict,
            )
            .with_code("conflict_already_resolved")
            .with_sqlstate("23505"),
        );
        let state = indexer_test_state(indexers)?;
        let request = IndexerSourceMetadataConflictResolveRequest {
            conflict_id: 7,
            resolution: "ignored".to_string(),
            resolution_note: Some("duplicate row".to_string()),
        };

        let error = resolve_source_metadata_conflict(State(state), Json(request))
            .await
            .expect_err("conflict error should map to API problem");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(CONFLICT_RESOLVE_FAILED));
        let context = problem.context.unwrap_or_default();
        assert!(
            context.iter().any(|field| field.name == "operation"
                && field.value == "source_metadata_conflict_resolve")
        );
        assert!(
            context
                .iter()
                .any(|field| field.name == "error_code"
                    && field.value == "conflict_already_resolved")
        );
        assert!(
            context
                .iter()
                .any(|field| field.name == "sqlstate" && field.value == "23505")
        );
        Ok(())
    }

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

    #[tokio::test]
    async fn reopen_source_metadata_conflict_maps_not_found_error() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers.set_source_metadata_conflict_error(
            SourceMetadataConflictServiceError::new(
                SourceMetadataConflictServiceErrorKind::NotFound,
            )
            .with_code("conflict_missing"),
        );
        let state = indexer_test_state(indexers)?;
        let request = IndexerSourceMetadataConflictReopenRequest {
            conflict_id: 7,
            resolution_note: Some("restore".to_string()),
        };

        let error = reopen_source_metadata_conflict(State(state), Json(request))
            .await
            .expect_err("missing conflict should map to not found");
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(CONFLICT_REOPEN_FAILED));
        let context = problem.context.unwrap_or_default();
        assert!(
            context.iter().any(|field| field.name == "operation"
                && field.value == "source_metadata_conflict_reopen")
        );
        assert!(
            context
                .iter()
                .any(|field| field.name == "error_code" && field.value == "conflict_missing")
        );
        Ok(())
    }
}
