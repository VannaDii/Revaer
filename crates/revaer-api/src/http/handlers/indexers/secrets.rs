//! Secret management endpoints for indexers.
//!
//! # Design
//! - Delegate secret lifecycle to the injected indexer facade.
//! - Avoid logging secret material; only log constant messages and identifiers.
//! - Surface RFC9457 problem documents with stable messages.

use std::sync::Arc;

use crate::app::indexers::{SecretServiceError, SecretServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::normalize_required_str_field;
use crate::models::{
    SecretCreateRequest, SecretResponse, SecretRevokeRequest, SecretRotateRequest,
};
use axum::{Json, extract::State, http::StatusCode};

const SECRET_CREATE_FAILED: &str = "failed to create secret";
const SECRET_ROTATE_FAILED: &str = "failed to rotate secret";
const SECRET_REVOKE_FAILED: &str = "failed to revoke secret";
const SECRET_TYPE_REQUIRED: &str = "secret_type is required";

pub(crate) async fn create_secret(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SecretCreateRequest>,
) -> Result<(StatusCode, Json<SecretResponse>), ApiError> {
    let secret_type = normalize_required_str_field(&request.secret_type, SECRET_TYPE_REQUIRED)?;
    let secret_public_id = state
        .indexers
        .secret_create(SYSTEM_ACTOR_PUBLIC_ID, secret_type, &request.secret_value)
        .await
        .map_err(|err| map_secret_error("secret_create", SECRET_CREATE_FAILED, &err))?;

    Ok((
        StatusCode::CREATED,
        Json(SecretResponse { secret_public_id }),
    ))
}

pub(crate) async fn rotate_secret(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SecretRotateRequest>,
) -> Result<Json<SecretResponse>, ApiError> {
    let secret_public_id = state
        .indexers
        .secret_rotate(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.secret_public_id,
            &request.secret_value,
        )
        .await
        .map_err(|err| map_secret_error("secret_rotate", SECRET_ROTATE_FAILED, &err))?;

    Ok(Json(SecretResponse { secret_public_id }))
}

pub(crate) async fn revoke_secret(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SecretRevokeRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .secret_revoke(SYSTEM_ACTOR_PUBLIC_ID, request.secret_public_id)
        .await
        .map_err(|err| map_secret_error("secret_revoke", SECRET_REVOKE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_secret_error(
    operation: &'static str,
    detail: &'static str,
    err: &SecretServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        SecretServiceErrorKind::Invalid => ApiError::bad_request(detail),
        SecretServiceErrorKind::NotFound => ApiError::not_found(detail),
        SecretServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        SecretServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{SecretServiceError, SecretServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_secret_trims_secret_type_and_returns_payload() -> Result<(), ApiError> {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone()))?;
        let request = SecretCreateRequest {
            secret_type: " api_key ".to_string(),
            secret_value: "value".to_string(),
        };

        let (status, Json(response)) = create_secret(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(response.secret_public_id, Uuid::nil());

        let recorded = indexers.created.lock().expect("lock poisoned");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, "api_key");
        assert_eq!(recorded[0].1, "value");
        drop(recorded);
        Ok(())
    }

    #[tokio::test]
    async fn rotate_secret_returns_payload() -> Result<(), ApiError> {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone()))?;
        let secret_public_id = Uuid::new_v4();
        let request = SecretRotateRequest {
            secret_public_id,
            secret_value: "new-value".to_string(),
        };

        let Json(response) = rotate_secret(State(state), Json(request)).await?;
        assert_eq!(response.secret_public_id, secret_public_id);

        let recorded = indexers.rotated.lock().expect("lock poisoned");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, secret_public_id);
        assert_eq!(recorded[0].1, "new-value");
        drop(recorded);
        Ok(())
    }

    #[tokio::test]
    async fn revoke_secret_returns_no_content() -> Result<(), ApiError> {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone()))?;
        let secret_public_id = Uuid::new_v4();
        let request = SecretRevokeRequest { secret_public_id };

        let status = revoke_secret(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let recorded = indexers.revoked.lock().expect("lock poisoned");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], secret_public_id);
        drop(recorded);
        Ok(())
    }

    #[tokio::test]
    async fn secret_errors_map_to_problem_details() {
        let invalid_indexers = RecordingIndexers::default();
        invalid_indexers
            .secret_error
            .lock()
            .expect("lock poisoned")
            .replace(SecretServiceError::new(SecretServiceErrorKind::Invalid));
        let base_state = indexer_test_state(Arc::new(invalid_indexers)).expect("api state");
        let request = SecretCreateRequest {
            secret_type: "api_key".to_string(),
            secret_value: String::new(),
        };
        let err = create_secret(State(base_state.clone()), Json(request))
            .await
            .unwrap_err();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.title, "bad request");

        let not_found_indexers = RecordingIndexers::default();
        not_found_indexers
            .secret_error
            .lock()
            .expect("lock poisoned")
            .replace(SecretServiceError::new(SecretServiceErrorKind::NotFound));
        let state_not_found = indexer_test_state(Arc::new(not_found_indexers)).expect("api state");
        let request = SecretRotateRequest {
            secret_public_id: Uuid::new_v4(),
            secret_value: "value".to_string(),
        };
        let err = rotate_secret(State(state_not_found), Json(request))
            .await
            .unwrap_err();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(problem.title, "resource not found");

        let unauthorized_indexers = RecordingIndexers::default();
        unauthorized_indexers
            .secret_error
            .lock()
            .expect("lock poisoned")
            .replace(SecretServiceError::new(
                SecretServiceErrorKind::Unauthorized,
            ));
        let state_unauthorized =
            indexer_test_state(Arc::new(unauthorized_indexers)).expect("api state");
        let request = SecretRevokeRequest {
            secret_public_id: Uuid::new_v4(),
        };
        let err = revoke_secret(State(state_unauthorized), Json(request))
            .await
            .unwrap_err();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let problem = parse_problem(response).await;
        assert_eq!(problem.title, "authentication required");
    }

    #[tokio::test]
    async fn create_secret_requires_non_blank_secret_type() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;
        let err = create_secret(
            State(state),
            Json(SecretCreateRequest {
                secret_type: "   ".to_string(),
                secret_value: "value".to_string(),
            }),
        )
        .await
        .expect_err("blank secret type should fail");

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(SECRET_TYPE_REQUIRED));
        Ok(())
    }
}
