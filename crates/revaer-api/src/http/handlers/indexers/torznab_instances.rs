//! Torznab instance management endpoints.
//!
//! # Design
//! - Delegate lifecycle changes to the injected indexer facade.
//! - Avoid logging API keys; return only the plaintext value from the service.
//! - Surface RFC9457 problem documents with stable messages.

use std::sync::Arc;

use axum::{Json, extract::Path, extract::State, http::StatusCode};

use crate::app::indexers::{TorznabInstanceServiceError, TorznabInstanceServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    TorznabInstanceCreateRequest, TorznabInstanceListResponse, TorznabInstanceResponse,
    TorznabInstanceStateRequest,
};
use uuid::Uuid;

const TORZNAB_INSTANCE_LIST_FAILED: &str = "failed to list torznab instances";
const TORZNAB_INSTANCE_CREATE_FAILED: &str = "failed to create torznab instance";
const TORZNAB_INSTANCE_ROTATE_FAILED: &str = "failed to rotate torznab api key";
const TORZNAB_INSTANCE_STATE_FAILED: &str = "failed to update torznab instance state";
const TORZNAB_INSTANCE_DELETE_FAILED: &str = "failed to delete torznab instance";

pub(crate) async fn create_torznab_instance(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TorznabInstanceCreateRequest>,
) -> Result<(StatusCode, Json<TorznabInstanceResponse>), ApiError> {
    let display_name = request.display_name.trim();
    let credentials = state
        .indexers
        .torznab_instance_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.search_profile_public_id,
            display_name,
        )
        .await
        .map_err(|err| {
            map_torznab_instance_error(
                "torznab_instance_create",
                TORZNAB_INSTANCE_CREATE_FAILED,
                &err,
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(TorznabInstanceResponse {
            torznab_instance_public_id: credentials.torznab_instance_public_id,
            api_key_plaintext: credentials.api_key_plaintext,
        }),
    ))
}

pub(crate) async fn list_torznab_instances(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<TorznabInstanceListResponse>, ApiError> {
    let torznab_instances = state
        .indexers
        .torznab_instance_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| {
            map_torznab_instance_error("torznab_instance_list", TORZNAB_INSTANCE_LIST_FAILED, &err)
        })?;

    Ok(Json(TorznabInstanceListResponse { torznab_instances }))
}

pub(crate) async fn rotate_torznab_instance_key(
    State(state): State<Arc<ApiState>>,
    Path(torznab_instance_public_id): Path<Uuid>,
) -> Result<Json<TorznabInstanceResponse>, ApiError> {
    let credentials = state
        .indexers
        .torznab_instance_rotate_key(SYSTEM_ACTOR_PUBLIC_ID, torznab_instance_public_id)
        .await
        .map_err(|err| {
            map_torznab_instance_error(
                "torznab_instance_rotate_key",
                TORZNAB_INSTANCE_ROTATE_FAILED,
                &err,
            )
        })?;

    Ok(Json(TorznabInstanceResponse {
        torznab_instance_public_id: credentials.torznab_instance_public_id,
        api_key_plaintext: credentials.api_key_plaintext,
    }))
}

pub(crate) async fn set_torznab_instance_state(
    State(state): State<Arc<ApiState>>,
    Path(torznab_instance_public_id): Path<Uuid>,
    Json(request): Json<TorznabInstanceStateRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .torznab_instance_enable_disable(
            SYSTEM_ACTOR_PUBLIC_ID,
            torznab_instance_public_id,
            request.is_enabled,
        )
        .await
        .map_err(|err| {
            map_torznab_instance_error(
                "torznab_instance_enable_disable",
                TORZNAB_INSTANCE_STATE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_torznab_instance(
    State(state): State<Arc<ApiState>>,
    Path(torznab_instance_public_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .torznab_instance_soft_delete(SYSTEM_ACTOR_PUBLIC_ID, torznab_instance_public_id)
        .await
        .map_err(|err| {
            map_torznab_instance_error(
                "torznab_instance_soft_delete",
                TORZNAB_INSTANCE_DELETE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_torznab_instance_error(
    operation: &'static str,
    detail: &'static str,
    err: &TorznabInstanceServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        TorznabInstanceServiceErrorKind::Invalid => ApiError::bad_request(detail),
        TorznabInstanceServiceErrorKind::NotFound => ApiError::not_found(detail),
        TorznabInstanceServiceErrorKind::Conflict => ApiError::conflict(detail),
        TorznabInstanceServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        TorznabInstanceServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{TorznabInstanceCredentials, TorznabInstanceServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::TorznabInstanceListItemResponse;
    use axum::response::IntoResponse;
    use std::sync::Arc;

    #[tokio::test]
    async fn create_torznab_instance_trims_name_and_returns_payload() -> Result<(), ApiError> {
        let search_profile_public_id = Uuid::new_v4();
        let instance_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .torznab_instance_create_result
            .lock()
            .expect("lock") = Some(Ok(TorznabInstanceCredentials {
            torznab_instance_public_id: instance_public_id,
            api_key_plaintext: "test-key".to_string(),
        }));
        let api = indexer_test_state(indexers.clone())?;

        let response = create_torznab_instance(
            State(api),
            Json(TorznabInstanceCreateRequest {
                search_profile_public_id,
                display_name: "  Torznab  ".to_string(),
            }),
        )
        .await?;

        assert_eq!(response.0, StatusCode::CREATED);
        assert_eq!(response.1.0.torznab_instance_public_id, instance_public_id);
        assert_eq!(response.1.0.api_key_plaintext, "test-key");
        assert_eq!(
            indexers
                .torznab_instance_create_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                search_profile_public_id,
                "Torznab".to_string()
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_torznab_instance_conflict_maps_problem() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .torznab_instance_create_result
            .lock()
            .expect("lock") = Some(Err(TorznabInstanceServiceError::new(
            TorznabInstanceServiceErrorKind::Conflict,
        )
        .with_code("display_name_already_exists")));
        let api = indexer_test_state(indexers)?;

        let response = create_torznab_instance(
            State(api),
            Json(TorznabInstanceCreateRequest {
                search_profile_public_id: Uuid::new_v4(),
                display_name: "Torznab".to_string(),
            }),
        )
        .await
        .expect_err("conflict should fail")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem = parse_problem(response).await;
        assert_eq!(
            problem.detail,
            Some(TORZNAB_INSTANCE_CREATE_FAILED.to_string())
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_torznab_instances_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .torznab_instance_list_items
            .lock()
            .expect("lock")
            .push(TorznabInstanceListItemResponse {
                torznab_instance_public_id: Uuid::new_v4(),
                display_name: "Docs".to_string(),
                is_enabled: true,
                search_profile_public_id: Uuid::new_v4(),
                search_profile_display_name: "Default".to_string(),
            });
        let api = indexer_test_state(indexers)?;

        let Json(response) = list_torznab_instances(State(api)).await?;
        assert_eq!(response.torznab_instances.len(), 1);
        assert_eq!(response.torznab_instances[0].display_name, "Docs");
        Ok(())
    }

    #[tokio::test]
    async fn rotate_torznab_instance_key_returns_payload() -> Result<(), ApiError> {
        let torznab_instance_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .torznab_instance_rotate_result
            .lock()
            .expect("lock") = Some(Ok(TorznabInstanceCredentials {
            torznab_instance_public_id,
            api_key_plaintext: "rotated-key".to_string(),
        }));
        let api = indexer_test_state(indexers.clone())?;

        let Json(response) =
            rotate_torznab_instance_key(State(api), Path(torznab_instance_public_id)).await?;

        assert_eq!(
            response.torznab_instance_public_id,
            torznab_instance_public_id
        );
        assert_eq!(response.api_key_plaintext, "rotated-key");
        assert_eq!(
            indexers
                .torznab_instance_rotate_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, torznab_instance_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_torznab_instance_state_records_requested_value() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let torznab_instance_public_id = Uuid::new_v4();
        let api = indexer_test_state(indexers.clone())?;

        let status = set_torznab_instance_state(
            State(api),
            Path(torznab_instance_public_id),
            Json(TorznabInstanceStateRequest { is_enabled: false }),
        )
        .await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            indexers
                .torznab_instance_state_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, torznab_instance_public_id, false)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_torznab_instance_records_identifier() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let torznab_instance_public_id = Uuid::new_v4();
        let api = indexer_test_state(indexers.clone())?;

        let status = delete_torznab_instance(State(api), Path(torznab_instance_public_id)).await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            indexers
                .torznab_instance_delete_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, torznab_instance_public_id)]
        );
        Ok(())
    }
}
