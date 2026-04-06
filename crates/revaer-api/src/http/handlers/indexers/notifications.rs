//! Health notification hook endpoints for indexer operators.
//!
//! # Design
//! - Delegate hook CRUD to the injected indexer facade.
//! - Keep request trimming and response shaping local to the HTTP boundary.
//! - Surface stable RFC9457 errors with structured context fields.

use std::sync::Arc;

use crate::app::indexers::{
    HealthNotificationHookUpdateParams, HealthNotificationServiceError,
    HealthNotificationServiceErrorKind,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{
    IndexerHealthNotificationHookCreateRequest, IndexerHealthNotificationHookDeleteRequest,
    IndexerHealthNotificationHookListResponse, IndexerHealthNotificationHookResponse,
    IndexerHealthNotificationHookUpdateRequest,
};
use axum::{Json, extract::State, http::StatusCode};

const HOOK_LIST_FAILED: &str = "failed to list health notification hooks";
const HOOK_GET_FAILED: &str = "failed to get health notification hook";
const HOOK_CREATE_FAILED: &str = "failed to create health notification hook";
const HOOK_UPDATE_FAILED: &str = "failed to update health notification hook";
const HOOK_DELETE_FAILED: &str = "failed to delete health notification hook";
const HOOK_CHANNEL_REQUIRED: &str = "channel is required";
const HOOK_DISPLAY_NAME_REQUIRED: &str = "display_name is required";
const HOOK_STATUS_THRESHOLD_REQUIRED: &str = "status_threshold is required";

pub(crate) async fn list_health_notification_hooks(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IndexerHealthNotificationHookListResponse>, ApiError> {
    let hooks = state
        .indexers
        .indexer_health_notification_hook_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| map_health_notification_error("hook_list", HOOK_LIST_FAILED, &err))?;
    Ok(Json(IndexerHealthNotificationHookListResponse { hooks }))
}

pub(crate) async fn create_health_notification_hook(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerHealthNotificationHookCreateRequest>,
) -> Result<(StatusCode, Json<IndexerHealthNotificationHookResponse>), ApiError> {
    let channel = normalize_required_str_field(&request.channel, HOOK_CHANNEL_REQUIRED)?;
    let display_name =
        normalize_required_str_field(&request.display_name, HOOK_DISPLAY_NAME_REQUIRED)?;
    let status_threshold =
        normalize_required_str_field(&request.status_threshold, HOOK_STATUS_THRESHOLD_REQUIRED)?;
    let webhook_url = trim_and_filter_empty(request.webhook_url.as_deref());
    let email = trim_and_filter_empty(request.email.as_deref());

    let hook_public_id = state
        .indexers
        .indexer_health_notification_hook_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            channel,
            display_name,
            status_threshold,
            webhook_url,
            email,
        )
        .await
        .map_err(|err| map_health_notification_error("hook_create", HOOK_CREATE_FAILED, &err))?;
    let response = load_hook_response(&state, hook_public_id).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

pub(crate) async fn update_health_notification_hook(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerHealthNotificationHookUpdateRequest>,
) -> Result<Json<IndexerHealthNotificationHookResponse>, ApiError> {
    let display_name = trim_and_filter_empty(request.display_name.as_deref());
    let status_threshold = trim_and_filter_empty(request.status_threshold.as_deref());
    let webhook_url = trim_and_filter_empty(request.webhook_url.as_deref());
    let email = trim_and_filter_empty(request.email.as_deref());

    let hook_public_id = state
        .indexers
        .indexer_health_notification_hook_update(HealthNotificationHookUpdateParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            hook_public_id: request.indexer_health_notification_hook_public_id,
            display_name,
            status_threshold,
            webhook_url,
            email,
            is_enabled: request.is_enabled,
        })
        .await
        .map_err(|err| map_health_notification_error("hook_update", HOOK_UPDATE_FAILED, &err))?;
    Ok(Json(load_hook_response(&state, hook_public_id).await?))
}

pub(crate) async fn delete_health_notification_hook(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerHealthNotificationHookDeleteRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .indexer_health_notification_hook_delete(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.indexer_health_notification_hook_public_id,
        )
        .await
        .map_err(|err| map_health_notification_error("hook_delete", HOOK_DELETE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_health_notification_error(
    operation: &'static str,
    detail: &'static str,
    err: &HealthNotificationServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        HealthNotificationServiceErrorKind::Invalid => ApiError::bad_request(detail),
        HealthNotificationServiceErrorKind::NotFound => ApiError::not_found(detail),
        HealthNotificationServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        HealthNotificationServiceErrorKind::Storage => ApiError::internal(detail),
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

async fn load_hook_response(
    state: &Arc<ApiState>,
    hook_public_id: uuid::Uuid,
) -> Result<IndexerHealthNotificationHookResponse, ApiError> {
    state
        .indexers
        .indexer_health_notification_hook_get(SYSTEM_ACTOR_PUBLIC_ID, hook_public_id)
        .await
        .map_err(|err| map_health_notification_error("hook_get", HOOK_GET_FAILED, &err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::HealthNotificationServiceErrorKind;
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use axum::response::IntoResponse;
    use std::sync::Arc;

    #[tokio::test]
    async fn list_health_notification_hooks_returns_payload() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state should build");
        let hook_public_id = uuid::Uuid::new_v4();
        indexers
            .health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(IndexerHealthNotificationHookResponse {
                indexer_health_notification_hook_public_id: hook_public_id,
                channel: "webhook".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: Some("https://hooks.example.test/pager".to_string()),
                email: None,
                is_enabled: true,
                updated_at: chrono::Utc::now(),
            });

        let Json(response) = list_health_notification_hooks(State(state))
            .await
            .expect("list should succeed");
        assert_eq!(response.hooks.len(), 1);
        assert_eq!(
            response.hooks[0].indexer_health_notification_hook_public_id,
            hook_public_id
        );
    }

    #[tokio::test]
    async fn create_health_notification_hook_maps_validation_errors() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state should build");
        *indexers
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(
            HealthNotificationServiceError::new(HealthNotificationServiceErrorKind::Invalid)
                .with_code("webhook_url_invalid")
                .with_sqlstate("P0001"),
        );

        let err = create_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookCreateRequest {
                channel: "webhook".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: Some("ftp://invalid".to_string()),
                email: None,
            }),
        )
        .await
        .expect_err("invalid hook create should fail");

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(HOOK_CREATE_FAILED));
        assert_eq!(
            problem
                .context
                .as_ref()
                .and_then(|fields| fields.iter().find(|field| field.name == "error_code"))
                .map(|field| field.value.as_str()),
            Some("webhook_url_invalid")
        );
    }

    #[tokio::test]
    async fn create_health_notification_hook_filters_whitespace_only_optionals() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state should build");

        let (_status, Json(response)) = create_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookCreateRequest {
                channel: "email".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "degraded".to_string(),
                webhook_url: Some("   ".to_string()),
                email: Some("   ".to_string()),
            }),
        )
        .await
        .expect("create should succeed");

        assert_eq!(response.webhook_url, None);
        assert_eq!(response.email, None);
    }

    #[tokio::test]
    async fn create_health_notification_hook_requires_non_blank_fields() {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default())).expect("state");

        let err = create_health_notification_hook(
            State(state.clone()),
            Json(IndexerHealthNotificationHookCreateRequest {
                channel: "   ".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: None,
                email: None,
            }),
        )
        .await
        .expect_err("blank channel should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(HOOK_CHANNEL_REQUIRED));

        let err = create_health_notification_hook(
            State(state.clone()),
            Json(IndexerHealthNotificationHookCreateRequest {
                channel: "webhook".to_string(),
                display_name: "   ".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: None,
                email: None,
            }),
        )
        .await
        .expect_err("blank display name should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(HOOK_DISPLAY_NAME_REQUIRED));

        let err = create_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookCreateRequest {
                channel: "webhook".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "   ".to_string(),
                webhook_url: None,
                email: None,
            }),
        )
        .await
        .expect_err("blank status threshold should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(
            problem.detail.as_deref(),
            Some(HOOK_STATUS_THRESHOLD_REQUIRED)
        );
    }

    #[tokio::test]
    async fn update_health_notification_hook_trims_optionals_and_returns_payload() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state should build");
        let hook_public_id = uuid::Uuid::new_v4();
        indexers
            .health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(IndexerHealthNotificationHookResponse {
                indexer_health_notification_hook_public_id: hook_public_id,
                channel: "webhook".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: Some("https://hooks.example.test/old".to_string()),
                email: Some("ops@example.test".to_string()),
                is_enabled: true,
                updated_at: chrono::Utc::now(),
            });

        let Json(response) = update_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookUpdateRequest {
                indexer_health_notification_hook_public_id: hook_public_id,
                display_name: Some("  Escalation Pager  ".to_string()),
                status_threshold: Some("  degraded  ".to_string()),
                webhook_url: Some("   ".to_string()),
                email: Some("  alerts@example.test  ".to_string()),
                is_enabled: Some(false),
            }),
        )
        .await
        .expect("update should succeed");

        assert_eq!(response.display_name, "Escalation Pager");
        assert_eq!(response.status_threshold, "degraded");
        assert_eq!(
            response.webhook_url,
            Some("https://hooks.example.test/old".to_string())
        );
        assert_eq!(response.email, Some("alerts@example.test".to_string()));
        assert!(!response.is_enabled);
    }

    #[tokio::test]
    async fn delete_health_notification_hook_returns_no_content() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone()).expect("state should build");
        let hook_public_id = uuid::Uuid::new_v4();
        indexers
            .health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(IndexerHealthNotificationHookResponse {
                indexer_health_notification_hook_public_id: hook_public_id,
                channel: "webhook".to_string(),
                display_name: "Pager".to_string(),
                status_threshold: "failing".to_string(),
                webhook_url: Some("https://hooks.example.test/pager".to_string()),
                email: None,
                is_enabled: true,
                updated_at: chrono::Utc::now(),
            });

        let status = delete_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookDeleteRequest {
                indexer_health_notification_hook_public_id: hook_public_id,
            }),
        )
        .await
        .expect("delete should succeed");

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(
            indexers
                .health_notification_hooks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
    }

    #[tokio::test]
    async fn delete_health_notification_hook_maps_not_found() {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default())).expect("state");
        let err = delete_health_notification_hook(
            State(state),
            Json(IndexerHealthNotificationHookDeleteRequest {
                indexer_health_notification_hook_public_id: uuid::Uuid::new_v4(),
            }),
        )
        .await
        .expect_err("missing hook should fail");

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(HOOK_DELETE_FAILED));
        assert_eq!(
            problem
                .context
                .as_ref()
                .and_then(|fields| fields.iter().find(|field| field.name == "error_code"))
                .map(|field| field.value.as_str()),
            Some("hook_not_found")
        );
    }
}
