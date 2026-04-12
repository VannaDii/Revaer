//! Import job endpoints for indexers.
//!
//! # Design
//! - Delegate import job operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::app::indexers::{ImportJobServiceError, ImportJobServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    ImportJobCreateRequest, ImportJobResponse, ImportJobResultsResponse,
    ImportJobRunProwlarrApiRequest, ImportJobRunProwlarrBackupRequest, ImportJobStatusResponse,
};

const IMPORT_JOB_CREATE_FAILED: &str = "failed to create import job";
const IMPORT_JOB_RUN_PROWLARR_API_FAILED: &str = "failed to start import job";
const IMPORT_JOB_RUN_PROWLARR_BACKUP_FAILED: &str = "failed to start import job";
const IMPORT_JOB_STATUS_FAILED: &str = "failed to fetch import job status";
const IMPORT_JOB_RESULTS_FAILED: &str = "failed to fetch import job results";

#[tracing::instrument(name = "indexer.job.import.create", skip(state, request))]
pub(crate) async fn create_import_job(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ImportJobCreateRequest>,
) -> Result<(StatusCode, Json<ImportJobResponse>), ApiError> {
    let source = request.source.trim();
    let import_job_public_id = match state
        .indexers
        .import_job_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            source,
            request.is_dry_run,
            request.target_search_profile_public_id,
            request.target_torznab_instance_public_id,
        )
        .await
    {
        Ok(import_job_public_id) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_create", "success");
            import_job_public_id
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_create", "error");
            return Err(map_import_job_error(
                "import_job_create",
                IMPORT_JOB_CREATE_FAILED,
                &err,
            ));
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(ImportJobResponse {
            import_job_public_id,
        }),
    ))
}

#[tracing::instrument(
    name = "indexer.job.import.run.prowlarr_api",
    skip(state, request),
    fields(import_job_public_id = %import_job_public_id)
)]
pub(crate) async fn run_import_job_prowlarr_api(
    Path(import_job_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ImportJobRunProwlarrApiRequest>,
) -> Result<StatusCode, ApiError> {
    let prowlarr_url = request.prowlarr_url.trim();
    match state
        .indexers
        .import_job_run_prowlarr_api(
            import_job_public_id,
            prowlarr_url,
            request.prowlarr_api_key_secret_public_id,
        )
        .await
    {
        Ok(()) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_run_prowlarr_api", "success");
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_run_prowlarr_api", "error");
            return Err(map_import_job_error(
                "import_job_run_prowlarr_api",
                IMPORT_JOB_RUN_PROWLARR_API_FAILED,
                &err,
            ));
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(
    name = "indexer.job.import.run.prowlarr_backup",
    skip(state, request),
    fields(import_job_public_id = %import_job_public_id)
)]
pub(crate) async fn run_import_job_prowlarr_backup(
    Path(import_job_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<ImportJobRunProwlarrBackupRequest>,
) -> Result<StatusCode, ApiError> {
    let backup_blob_ref = request.backup_blob_ref.trim();
    match state
        .indexers
        .import_job_run_prowlarr_backup(import_job_public_id, backup_blob_ref)
        .await
    {
        Ok(()) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_run_prowlarr_backup", "success");
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_run_prowlarr_backup", "error");
            return Err(map_import_job_error(
                "import_job_run_prowlarr_backup",
                IMPORT_JOB_RUN_PROWLARR_BACKUP_FAILED,
                &err,
            ));
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(
    name = "indexer.job.import.status",
    skip(state),
    fields(import_job_public_id = %import_job_public_id)
)]
pub(crate) async fn get_import_job_status(
    Path(import_job_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ImportJobStatusResponse>, ApiError> {
    let status = match state
        .indexers
        .import_job_get_status(import_job_public_id)
        .await
    {
        Ok(status) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_status", "success");
            status
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_status", "error");
            return Err(map_import_job_error(
                "import_job_get_status",
                IMPORT_JOB_STATUS_FAILED,
                &err,
            ));
        }
    };

    Ok(Json(status))
}

#[tracing::instrument(
    name = "indexer.job.import.results",
    skip(state),
    fields(import_job_public_id = %import_job_public_id)
)]
pub(crate) async fn list_import_job_results(
    Path(import_job_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ImportJobResultsResponse>, ApiError> {
    let results = match state
        .indexers
        .import_job_list_results(import_job_public_id)
        .await
    {
        Ok(results) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_results", "success");
            results
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_job_outcome("import_results", "error");
            return Err(map_import_job_error(
                "import_job_list_results",
                IMPORT_JOB_RESULTS_FAILED,
                &err,
            ));
        }
    };

    Ok(Json(ImportJobResultsResponse { results }))
}

fn map_import_job_error(
    operation: &'static str,
    detail: &'static str,
    err: &ImportJobServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        ImportJobServiceErrorKind::Invalid => ApiError::bad_request(detail),
        ImportJobServiceErrorKind::NotFound => ApiError::not_found(detail),
        ImportJobServiceErrorKind::Conflict => ApiError::conflict(detail),
        ImportJobServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        ImportJobServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{ImportJobServiceError, ImportJobServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{ImportJobResultResponse, ProblemDetails};
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_import_job_trims_source_and_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let search_profile_public_id = Uuid::new_v4();
        let torznab_instance_public_id = Uuid::new_v4();
        let request = ImportJobCreateRequest {
            source: " prowlarr_api ".to_string(),
            is_dry_run: Some(true),
            target_search_profile_public_id: Some(search_profile_public_id),
            target_torznab_instance_public_id: Some(torznab_instance_public_id),
        };

        let (status, Json(response)) = create_import_job(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(response.import_job_public_id, Uuid::nil());

        let calls = indexers
            .import_job_create_calls
            .lock()
            .expect("lock")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                "prowlarr_api".to_string(),
                Some(true),
                Some(search_profile_public_id),
                Some(torznab_instance_public_id),
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_import_job_conflict_maps_problem_context() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.import_job_create_error.lock().expect("lock") = Some(
            ImportJobServiceError::new(ImportJobServiceErrorKind::Conflict)
                .with_code("import_job_exists")
                .with_sqlstate("23505"),
        );
        let state = indexer_test_state(indexers)?;

        let err = create_import_job(
            State(state),
            Json(ImportJobCreateRequest {
                source: "prowlarr_api".to_string(),
                is_dry_run: Some(false),
                target_search_profile_public_id: None,
                target_torznab_instance_public_id: None,
            }),
        )
        .await
        .expect_err("conflicting import job should map to problem details");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem: ProblemDetails = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "operation"));
        assert!(context.iter().any(|field| field.name == "error_code"));
        assert!(context.iter().any(|field| field.name == "sqlstate"));
        Ok(())
    }

    #[tokio::test]
    async fn run_import_job_trims_prowlarr_url() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let secret_public_id = Uuid::new_v4();
        let request = ImportJobRunProwlarrApiRequest {
            prowlarr_url: " http://localhost:9696 ".to_string(),
            prowlarr_api_key_secret_public_id: secret_public_id,
        };
        let job_id = Uuid::new_v4();

        let status = run_import_job_prowlarr_api(Path(job_id), State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .import_job_run_prowlarr_api_calls
            .lock()
            .expect("lock")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(
                job_id,
                "http://localhost:9696".to_string(),
                secret_public_id
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn run_import_job_trims_backup_blob_ref() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let job_id = Uuid::new_v4();

        let status = run_import_job_prowlarr_backup(
            Path(job_id),
            State(state),
            Json(ImportJobRunProwlarrBackupRequest {
                backup_blob_ref: " snapshot-blob ".to_string(),
            }),
        )
        .await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .import_job_run_prowlarr_backup_calls
            .lock()
            .expect("lock")
            .clone();
        assert_eq!(calls.as_slice(), &[(job_id, "snapshot-blob".to_string())]);
        Ok(())
    }

    #[tokio::test]
    async fn import_job_not_found_maps_not_found() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.import_job_status_error.lock().expect("lock") = Some(
            ImportJobServiceError::new(ImportJobServiceErrorKind::NotFound)
                .with_code("import_job_not_found"),
        );
        let state = indexer_test_state(indexers)?;

        let err = get_import_job_status(Path(Uuid::new_v4()), State(state))
            .await
            .expect_err("missing import job should map to not found");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem: ProblemDetails = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }

    #[tokio::test]
    async fn get_import_job_status_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.import_job_status_response.lock().expect("lock") =
            Some(ImportJobStatusResponse {
                status: "running".to_string(),
                result_total: 7,
                result_imported_ready: 3,
                result_imported_needs_secret: 1,
                result_imported_test_failed: 1,
                result_unmapped_definition: 1,
                result_skipped_duplicate: 1,
            });
        let state = indexer_test_state(indexers)?;

        let Json(status) = get_import_job_status(Path(Uuid::new_v4()), State(state)).await?;
        assert_eq!(status.status, "running");
        assert_eq!(status.result_total, 7);
        assert_eq!(status.result_imported_ready, 3);
        assert_eq!(status.result_imported_needs_secret, 1);
        Ok(())
    }

    #[tokio::test]
    async fn list_import_job_results_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.import_job_results_response.lock().expect("lock") =
            Some(vec![ImportJobResultResponse {
                prowlarr_identifier: "id".to_string(),
                upstream_slug: Some("demo".to_string()),
                indexer_instance_public_id: Some(Uuid::new_v4()),
                status: "imported_ready".to_string(),
                detail: Some("ready".to_string()),
                resolved_is_enabled: Some(true),
                resolved_priority: Some(50),
                missing_secret_fields: 0,
                media_domain_keys: vec!["movies".to_string()],
                tag_keys: vec!["favorites".to_string()],
                created_at: chrono::Utc::now(),
            }]);
        let state = indexer_test_state(indexers)?;

        let Json(response) = list_import_job_results(Path(Uuid::new_v4()), State(state)).await?;
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].status, "imported_ready");
        assert_eq!(response.results[0].detail.as_deref(), Some("ready"));
        assert_eq!(
            response.results[0].media_domain_keys,
            vec!["movies".to_string()]
        );
        assert_eq!(response.results[0].tag_keys, vec!["favorites".to_string()]);
        Ok(())
    }
}
