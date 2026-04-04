//! Indexer backup export and restore endpoints.
//!
//! # Design
//! - Delegate snapshot export and restore to the injected indexer facade.
//! - Keep response messages constant while surfacing structured error codes.
//! - Use the seeded system actor for operator-triggered backup flows.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};

use crate::app::indexers::{IndexerBackupServiceError, IndexerBackupServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    IndexerBackupExportResponse, IndexerBackupRestoreRequest, IndexerBackupRestoreResponse,
};

const INDEXER_BACKUP_EXPORT_FAILED: &str = "failed to export indexer backup";
const INDEXER_BACKUP_RESTORE_FAILED: &str = "failed to restore indexer backup";

#[tracing::instrument(name = "indexer.backup.export", skip(state))]
pub(crate) async fn export_indexer_backup(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IndexerBackupExportResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_backup_export(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|error| {
            map_indexer_backup_error(
                "indexer_backup_export",
                INDEXER_BACKUP_EXPORT_FAILED,
                &error,
            )
        })?;

    Ok(Json(response))
}

#[tracing::instrument(name = "indexer.backup.restore", skip(state, request))]
pub(crate) async fn restore_indexer_backup(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerBackupRestoreRequest>,
) -> Result<(StatusCode, Json<IndexerBackupRestoreResponse>), ApiError> {
    let response = state
        .indexers
        .indexer_backup_restore(SYSTEM_ACTOR_PUBLIC_ID, &request.snapshot)
        .await
        .map_err(|error| {
            map_indexer_backup_error(
                "indexer_backup_restore",
                INDEXER_BACKUP_RESTORE_FAILED,
                &error,
            )
        })?;

    Ok((StatusCode::OK, Json(response)))
}

fn map_indexer_backup_error(
    operation: &'static str,
    detail: &'static str,
    err: &IndexerBackupServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        IndexerBackupServiceErrorKind::Invalid => ApiError::bad_request(detail),
        IndexerBackupServiceErrorKind::Conflict => ApiError::conflict(detail),
        IndexerBackupServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        IndexerBackupServiceErrorKind::NotFound => ApiError::not_found(detail),
        IndexerBackupServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{IndexerBackupServiceError, IndexerBackupServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{
        IndexerBackupExportResponse, IndexerBackupRestoreRequest, IndexerBackupRestoreResponse,
        IndexerBackupSnapshot,
    };
    use axum::extract::State;
    use axum::response::IntoResponse;
    use chrono::Utc;

    #[tokio::test]
    async fn export_returns_snapshot_payload() -> anyhow::Result<()> {
        let indexers = RecordingIndexers::default();
        let snapshot = IndexerBackupSnapshot {
            version: "revaer.indexers.backup.v1".to_string(),
            exported_at: Utc::now(),
            tags: Vec::new(),
            rate_limit_policies: Vec::new(),
            routing_policies: Vec::new(),
            indexer_instances: Vec::new(),
            secrets: Vec::new(),
        };
        *indexers.backup_export_response.lock().expect("lock") =
            Some(IndexerBackupExportResponse { snapshot });

        let state = indexer_test_state(Arc::new(indexers))?;
        let Json(response) = export_indexer_backup(State(state)).await?;
        assert_eq!(response.snapshot.version, "revaer.indexers.backup.v1");
        Ok(())
    }

    #[tokio::test]
    async fn restore_returns_summary_payload() -> anyhow::Result<()> {
        let indexers = RecordingIndexers::default();
        *indexers.backup_restore_response.lock().expect("lock") =
            Some(IndexerBackupRestoreResponse {
                created_tag_count: 1,
                created_rate_limit_policy_count: 2,
                created_routing_policy_count: 3,
                created_indexer_instance_count: 4,
                unresolved_secret_bindings: Vec::new(),
            });
        let state = indexer_test_state(Arc::new(indexers))?;
        let request = IndexerBackupRestoreRequest {
            snapshot: IndexerBackupSnapshot {
                version: "revaer.indexers.backup.v1".to_string(),
                exported_at: Utc::now(),
                tags: Vec::new(),
                rate_limit_policies: Vec::new(),
                routing_policies: Vec::new(),
                indexer_instances: Vec::new(),
                secrets: Vec::new(),
            },
        };

        let (status, Json(response)) = restore_indexer_backup(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.created_indexer_instance_count, 4);
        Ok(())
    }

    #[tokio::test]
    async fn export_maps_conflict_details() -> anyhow::Result<()> {
        let indexers = RecordingIndexers::default();
        *indexers.backup_export_error.lock().expect("lock") = Some(
            IndexerBackupServiceError::new(IndexerBackupServiceErrorKind::Conflict)
                .with_code("display_name_already_exists")
                .with_sqlstate("P0001"),
        );
        let state = indexer_test_state(Arc::new(indexers))?;

        let error = export_indexer_backup(State(state))
            .await
            .expect_err("expected backup export error");
        let problem = parse_problem(error.into_response()).await;
        assert_eq!(problem.status, StatusCode::CONFLICT.as_u16());
        assert_eq!(
            problem.context.as_ref().and_then(|fields| {
                fields
                    .iter()
                    .find(|field| field.name == "error_code")
                    .map(|field| field.value.as_str())
            }),
            Some("display_name_already_exists")
        );
        Ok(())
    }
}
