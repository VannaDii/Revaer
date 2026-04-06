//! Rate limit policy endpoints for indexers.
//!
//! # Design
//! - Delegate rate limit policy operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::app::indexers::{RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{
    RateLimitPolicyAssignmentRequest, RateLimitPolicyCreateRequest, RateLimitPolicyListResponse,
    RateLimitPolicyResponse, RateLimitPolicyUpdateRequest,
};

const RATE_LIMIT_LIST_FAILED: &str = "failed to fetch rate limit policies";
const RATE_LIMIT_CREATE_FAILED: &str = "failed to create rate limit policy";
const RATE_LIMIT_UPDATE_FAILED: &str = "failed to update rate limit policy";
const RATE_LIMIT_DELETE_FAILED: &str = "failed to delete rate limit policy";
const RATE_LIMIT_ASSIGN_FAILED: &str = "failed to assign rate limit policy";
const RATE_LIMIT_DISPLAY_NAME_REQUIRED: &str = "display_name is required";

pub(crate) async fn create_rate_limit_policy(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RateLimitPolicyCreateRequest>,
) -> Result<(StatusCode, Json<RateLimitPolicyResponse>), ApiError> {
    let display_name =
        normalize_required_str_field(&request.display_name, RATE_LIMIT_DISPLAY_NAME_REQUIRED)?;
    let rate_limit_policy_public_id = state
        .indexers
        .rate_limit_policy_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            display_name,
            request.rpm,
            request.burst,
            request.concurrent,
        )
        .await
        .map_err(|err| {
            map_rate_limit_error("rate_limit_policy_create", RATE_LIMIT_CREATE_FAILED, &err)
        })?;

    Ok((
        StatusCode::CREATED,
        Json(RateLimitPolicyResponse {
            rate_limit_policy_public_id,
        }),
    ))
}

pub(crate) async fn list_rate_limit_policies(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<RateLimitPolicyListResponse>, ApiError> {
    let rate_limit_policies = state
        .indexers
        .rate_limit_policy_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| {
            map_rate_limit_error("rate_limit_policy_list", RATE_LIMIT_LIST_FAILED, &err)
        })?;

    Ok(Json(RateLimitPolicyListResponse {
        rate_limit_policies,
    }))
}

pub(crate) async fn update_rate_limit_policy(
    Path(rate_limit_policy_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RateLimitPolicyUpdateRequest>,
) -> Result<StatusCode, ApiError> {
    let display_name = trim_and_filter_empty(request.display_name.as_deref());
    state
        .indexers
        .rate_limit_policy_update(
            SYSTEM_ACTOR_PUBLIC_ID,
            rate_limit_policy_public_id,
            display_name,
            request.rpm,
            request.burst,
            request.concurrent,
        )
        .await
        .map_err(|err| {
            map_rate_limit_error("rate_limit_policy_update", RATE_LIMIT_UPDATE_FAILED, &err)
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_rate_limit_policy(
    Path(rate_limit_policy_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .rate_limit_policy_soft_delete(SYSTEM_ACTOR_PUBLIC_ID, rate_limit_policy_public_id)
        .await
        .map_err(|err| {
            map_rate_limit_error(
                "rate_limit_policy_soft_delete",
                RATE_LIMIT_DELETE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_indexer_instance_rate_limit(
    Path(indexer_instance_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RateLimitPolicyAssignmentRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .indexer_instance_set_rate_limit_policy(
            SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            request.rate_limit_policy_public_id,
        )
        .await
        .map_err(|err| {
            map_rate_limit_error(
                "indexer_instance_set_rate_limit_policy",
                RATE_LIMIT_ASSIGN_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_routing_policy_rate_limit(
    Path(routing_policy_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RateLimitPolicyAssignmentRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .routing_policy_set_rate_limit_policy(
            SYSTEM_ACTOR_PUBLIC_ID,
            routing_policy_public_id,
            request.rate_limit_policy_public_id,
        )
        .await
        .map_err(|err| {
            map_rate_limit_error(
                "routing_policy_set_rate_limit_policy",
                RATE_LIMIT_ASSIGN_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_rate_limit_error(
    operation: &'static str,
    detail: &'static str,
    err: &RateLimitPolicyServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        RateLimitPolicyServiceErrorKind::Invalid => ApiError::bad_request(detail),
        RateLimitPolicyServiceErrorKind::NotFound => ApiError::not_found(detail),
        RateLimitPolicyServiceErrorKind::Conflict => ApiError::conflict(detail),
        RateLimitPolicyServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        RateLimitPolicyServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::RateLimitPolicyListItemResponse;
    use axum::response::IntoResponse;
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_rate_limit_policy_trims_name_and_returns_id() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let payload = json!({
            "display_name": "  Default  ",
            "rpm": 60,
            "burst": 10,
            "concurrent": 2
        });

        let response =
            create_rate_limit_policy(State(state), Json(serde_json::from_value(payload).unwrap()))
                .await
                .expect("create should succeed");

        assert_eq!(response.0, StatusCode::CREATED);
        assert_eq!(
            indexers
                .rate_limit_create_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[("Default".to_string(), 60, 10, 2)]
        );
    }

    #[tokio::test]
    async fn create_rate_limit_policy_rejects_blank_display_name() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers).expect("state");
        let payload = json!({
            "display_name": "   ",
            "rpm": 60,
            "burst": 10,
            "concurrent": 2
        });

        let response = create_rate_limit_policy(
            State(state),
            Json(serde_json::from_value(payload).expect("request")),
        )
        .await
        .expect_err("blank display name should fail")
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(
            problem.detail.as_deref(),
            Some(RATE_LIMIT_DISPLAY_NAME_REQUIRED)
        );
    }

    #[tokio::test]
    async fn update_rate_limit_policy_records_updates() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let payload = json!({
            "display_name": "  Updated  ",
            "rpm": 120,
            "burst": 20,
            "concurrent": 4
        });
        let policy_id = Uuid::new_v4();

        let response = update_rate_limit_policy(
            Path(policy_id),
            State(state),
            Json(serde_json::from_value(payload).unwrap()),
        )
        .await
        .expect("update should succeed");

        assert_eq!(response, StatusCode::NO_CONTENT);
        let updates = indexers
            .rate_limit_update_calls
            .lock()
            .expect("lock")
            .clone();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, policy_id);
        assert_eq!(updates[0].1.as_deref(), Some("Updated"));
        assert_eq!(updates[0].2, Some(120));
        assert_eq!(updates[0].3, Some(20));
        assert_eq!(updates[0].4, Some(4));
    }

    #[tokio::test]
    async fn update_rate_limit_policy_filters_whitespace_only_display_name() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let payload = json!({ "display_name": "   " });
        let policy_id = Uuid::new_v4();

        let response = update_rate_limit_policy(
            Path(policy_id),
            State(state),
            Json(serde_json::from_value(payload).unwrap()),
        )
        .await
        .expect("update should succeed");

        assert_eq!(response, StatusCode::NO_CONTENT);
        let updates = indexers
            .rate_limit_update_calls
            .lock()
            .expect("lock")
            .clone();
        assert_eq!(updates, vec![(policy_id, None, None, None, None)]);
    }

    #[tokio::test]
    async fn assign_rate_limit_policy_to_indexer_instance() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let policy_id = Uuid::new_v4();
        let instance_id = Uuid::new_v4();
        let payload = json!({ "rate_limit_policy_public_id": policy_id });

        let response = set_indexer_instance_rate_limit(
            Path(instance_id),
            State(state),
            Json(serde_json::from_value(payload).unwrap()),
        )
        .await
        .expect("assignment should succeed");

        assert_eq!(response, StatusCode::NO_CONTENT);
        assert_eq!(
            indexers
                .indexer_rate_limit_assignment_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(instance_id, Some(policy_id))]
        );
    }

    #[tokio::test]
    async fn list_rate_limit_policies_returns_payload() {
        let indexers = Arc::new(RecordingIndexers::default());
        let policy_id = Uuid::new_v4();
        indexers.rate_limit_list_items.lock().expect("lock").push(
            RateLimitPolicyListItemResponse {
                rate_limit_policy_public_id: policy_id,
                display_name: "Burst".to_string(),
                requests_per_minute: 120,
                burst: 20,
                concurrent_requests: 4,
                is_system: false,
            },
        );
        let state = indexer_test_state(indexers).expect("state");

        let Json(response) = list_rate_limit_policies(State(state))
            .await
            .expect("list should succeed");

        assert_eq!(response.rate_limit_policies.len(), 1);
        assert_eq!(
            response.rate_limit_policies[0].rate_limit_policy_public_id,
            policy_id
        );
        assert_eq!(response.rate_limit_policies[0].display_name, "Burst");
    }

    #[tokio::test]
    async fn delete_rate_limit_policy_records_identifier() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let policy_id = Uuid::new_v4();

        let response = delete_rate_limit_policy(Path(policy_id), State(state))
            .await
            .expect("delete should succeed");

        assert_eq!(response, StatusCode::NO_CONTENT);
        assert_eq!(
            indexers
                .rate_limit_deleted_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[policy_id]
        );
    }

    #[tokio::test]
    async fn assign_rate_limit_policy_to_routing_policy() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state");
        let policy_id = Uuid::new_v4();
        let routing_policy_id = Uuid::new_v4();
        let payload = json!({ "rate_limit_policy_public_id": policy_id });

        let response = set_routing_policy_rate_limit(
            Path(routing_policy_id),
            State(state),
            Json(serde_json::from_value(payload).expect("request")),
        )
        .await
        .expect("routing assignment should succeed");

        assert_eq!(response, StatusCode::NO_CONTENT);
        assert_eq!(
            indexers
                .routing_rate_limit_assignment_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(routing_policy_id, Some(policy_id))]
        );
    }

    #[tokio::test]
    async fn create_rate_limit_policy_surfaces_service_error_context() {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.rate_limit_create_error.lock().expect("lock") = Some(
            RateLimitPolicyServiceError::new(RateLimitPolicyServiceErrorKind::Conflict)
                .with_code("duplicate_rate_limit")
                .with_sqlstate("23505"),
        );
        let state = indexer_test_state(indexers).expect("state");
        let payload = json!({
            "display_name": "Default",
            "rpm": 60,
            "burst": 10,
            "concurrent": 2
        });

        let response = create_rate_limit_policy(
            State(state),
            Json(serde_json::from_value(payload).expect("request")),
        )
        .await
        .expect_err("service error should fail")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(RATE_LIMIT_CREATE_FAILED));
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| {
            field.name == "operation" && field.value == "rate_limit_policy_create"
        }));
        assert!(
            context.iter().any(|field| {
                field.name == "error_code" && field.value == "duplicate_rate_limit"
            })
        );
        assert!(
            context
                .iter()
                .any(|field| { field.name == "sqlstate" && field.value == "23505" })
        );
    }
}
