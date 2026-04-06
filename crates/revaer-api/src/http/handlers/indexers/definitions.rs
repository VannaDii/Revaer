//! Indexer definition catalog endpoints.
//!
//! # Design
//! - Delegate catalog reads to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use crate::app::indexers::{IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    CardigannDefinitionImportRequest, CardigannDefinitionImportResponse,
    IndexerDefinitionListResponse,
};
use axum::{Json, extract::State, http::StatusCode};

pub(crate) async fn list_indexer_definitions(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IndexerDefinitionListResponse>, ApiError> {
    const INDEXER_DEFINITION_LIST_FAILED: &str = "failed to list indexer definitions";
    let definitions = state
        .indexers
        .indexer_definition_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| {
            map_indexer_definition_error(
                "indexer_definition_list",
                INDEXER_DEFINITION_LIST_FAILED,
                &err,
            )
        })?;

    Ok(Json(IndexerDefinitionListResponse { definitions }))
}

pub(crate) async fn import_cardigann_definition(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CardigannDefinitionImportRequest>,
) -> Result<(StatusCode, Json<CardigannDefinitionImportResponse>), ApiError> {
    const INDEXER_DEFINITION_IMPORT_FAILED: &str = "failed to import Cardigann definition";
    let response = state
        .indexers
        .indexer_definition_import_cardigann(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.yaml_payload.trim(),
            request.is_deprecated,
        )
        .await
        .map_err(|err| {
            map_indexer_definition_error(
                "indexer_definition_import_cardigann",
                INDEXER_DEFINITION_IMPORT_FAILED,
                &err,
            )
        })?;

    Ok((StatusCode::CREATED, Json(response)))
}

fn map_indexer_definition_error(
    operation: &'static str,
    detail: &'static str,
    err: &IndexerDefinitionServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        IndexerDefinitionServiceErrorKind::Invalid => ApiError::bad_request(detail),
        IndexerDefinitionServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        IndexerDefinitionServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{CardigannDefinitionImportResponse, IndexerDefinitionResponse};
    use axum::response::IntoResponse;
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn list_indexer_definitions_returns_payload() -> Result<(), ApiError> {
        let definition = IndexerDefinitionResponse {
            upstream_source: "prowlarr_indexers".to_string(),
            upstream_slug: "alpha".to_string(),
            display_name: "Alpha".to_string(),
            protocol: "torrent".to_string(),
            engine: "torznab".to_string(),
            schema_version: 1,
            definition_hash: "a".repeat(64),
            is_deprecated: false,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap(),
        };
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .indexer_definition_list_result
            .lock()
            .expect("lock") = Some(Ok(vec![definition.clone()]));
        let state = indexer_test_state(indexers.clone())?;

        let Json(payload) = list_indexer_definitions(State(state)).await?;
        assert_eq!(payload.definitions, vec![definition]);

        assert_eq!(
            indexers
                .indexer_definition_list_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[SYSTEM_ACTOR_PUBLIC_ID]
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_indexer_definitions_maps_unauthorized() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .indexer_definition_list_result
            .lock()
            .expect("lock") = Some(Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Unauthorized,
        )
        .with_code("actor_missing")));
        let state = indexer_test_state(indexers)?;

        let err = list_indexer_definitions(State(state)).await.err().unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "operation"));
        Ok(())
    }

    #[tokio::test]
    async fn import_cardigann_definition_returns_payload() -> Result<(), ApiError> {
        let definition = IndexerDefinitionResponse {
            upstream_source: "cardigann".to_string(),
            upstream_slug: "example".to_string(),
            display_name: "Example".to_string(),
            protocol: "torrent".to_string(),
            engine: "cardigann".to_string(),
            schema_version: 1,
            definition_hash: "b".repeat(64),
            is_deprecated: false,
            created_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
        };
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .indexer_definition_import_result
            .lock()
            .expect("lock") = Some(Ok(CardigannDefinitionImportResponse {
            definition: definition.clone(),
            field_count: 2,
            option_count: 3,
        }));
        let state = indexer_test_state(indexers.clone())?;
        let request = CardigannDefinitionImportRequest {
            yaml_payload: "  id: example\nname: Example\nsettings: []  ".to_string(),
            is_deprecated: Some(false),
        };

        let (status, Json(payload)) =
            import_cardigann_definition(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(payload.definition, definition);
        assert_eq!(payload.field_count, 2);
        assert_eq!(
            indexers
                .indexer_definition_import_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                "id: example\nname: Example\nsettings: []".to_string(),
                Some(false),
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn import_cardigann_definition_maps_invalid() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .indexer_definition_import_result
            .lock()
            .expect("lock") = Some(Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Invalid,
        )
        .with_code("cardigann_yaml_invalid")));
        let state = indexer_test_state(indexers)?;
        let request = CardigannDefinitionImportRequest {
            yaml_payload: "not-yaml".to_string(),
            is_deprecated: Some(false),
        };

        let err = import_cardigann_definition(State(state), Json(request))
            .await
            .expect_err("expected invalid request error");
        let response = err.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }

    #[tokio::test]
    async fn import_cardigann_definition_maps_storage_sqlstate() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .indexer_definition_import_result
            .lock()
            .expect("lock") = Some(Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Storage,
        )
        .with_sqlstate("23505")));
        let state = indexer_test_state(indexers)?;
        let request = CardigannDefinitionImportRequest {
            yaml_payload: "id: example".to_string(),
            is_deprecated: None,
        };

        let err = import_cardigann_definition(State(state), Json(request))
            .await
            .expect_err("expected storage error");
        let response = err.into_response();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "sqlstate"));
        Ok(())
    }
}
