//! Tag management endpoints for indexers.
//!
//! # Design
//! - Delegate tag operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use crate::app::indexers::{TagServiceError, TagServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{
    TagCreateRequest, TagDeleteRequest, TagListResponse, TagResponse, TagUpdateRequest,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

const TAG_CREATE_FAILED: &str = "failed to create tag";
const TAG_LIST_FAILED: &str = "failed to list tags";
const TAG_UPDATE_FAILED: &str = "failed to update tag";
const TAG_DELETE_FAILED: &str = "failed to delete tag";
const TAG_REFERENCE_REQUIRED: &str = "tag identifier is required";
const TAG_KEY_REQUIRED: &str = "tag_key is required";
const TAG_DISPLAY_NAME_REQUIRED: &str = "display_name is required";

pub(crate) async fn create_tag(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TagCreateRequest>,
) -> Result<(StatusCode, Json<TagResponse>), ApiError> {
    let tag_key = normalize_required_str_field(&request.tag_key, TAG_KEY_REQUIRED)?;
    let display_name =
        normalize_required_str_field(&request.display_name, TAG_DISPLAY_NAME_REQUIRED)?;
    let tag_public_id = state
        .indexers
        .tag_create(SYSTEM_ACTOR_PUBLIC_ID, tag_key, display_name)
        .await
        .map_err(|err| map_tag_error("tag_create", TAG_CREATE_FAILED, &err))?;

    Ok((
        StatusCode::CREATED,
        Json(TagResponse {
            tag_public_id,
            tag_key: Some(tag_key.to_string()),
            display_name: display_name.to_string(),
        }),
    ))
}

pub(crate) async fn list_tags(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<TagListResponse>, ApiError> {
    let tags = state
        .indexers
        .tag_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| map_tag_error("tag_list", TAG_LIST_FAILED, &err))?;

    Ok(Json(TagListResponse { tags }))
}

pub(crate) async fn update_tag(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TagUpdateRequest>,
) -> Result<Json<TagResponse>, ApiError> {
    let tag_key = trim_and_filter_empty(request.tag_key.as_deref());
    if request.tag_public_id.is_none() && tag_key.is_none() {
        return Err(ApiError::bad_request(TAG_REFERENCE_REQUIRED));
    }
    let display_name = request.display_name.trim();
    let tag_public_id = state
        .indexers
        .tag_update(
            SYSTEM_ACTOR_PUBLIC_ID,
            request.tag_public_id,
            tag_key,
            display_name,
        )
        .await
        .map_err(|err| map_tag_error("tag_update", TAG_UPDATE_FAILED, &err))?;

    Ok(Json(TagResponse {
        tag_public_id,
        tag_key: tag_key.map(str::to_string),
        display_name: display_name.to_string(),
    }))
}

pub(crate) async fn delete_tag(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TagDeleteRequest>,
) -> Result<StatusCode, ApiError> {
    let tag_key = trim_and_filter_empty(request.tag_key.as_deref());
    if request.tag_public_id.is_none() && tag_key.is_none() {
        return Err(ApiError::bad_request(TAG_REFERENCE_REQUIRED));
    }
    state
        .indexers
        .tag_delete(SYSTEM_ACTOR_PUBLIC_ID, request.tag_public_id, tag_key)
        .await
        .map_err(|err| map_tag_error("tag_delete", TAG_DELETE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_tag_by_key(
    State(state): State<Arc<ApiState>>,
    Path(tag_key): Path<String>,
) -> Result<StatusCode, ApiError> {
    let trimmed_tag_key = trim_and_filter_empty(Some(tag_key.as_str()));
    if trimmed_tag_key.is_none() {
        return Err(ApiError::bad_request(TAG_REFERENCE_REQUIRED));
    }
    state
        .indexers
        .tag_delete(SYSTEM_ACTOR_PUBLIC_ID, None, trimmed_tag_key)
        .await
        .map_err(|err| map_tag_error("tag_delete_by_key", TAG_DELETE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_tag_error(operation: &'static str, detail: &'static str, err: &TagServiceError) -> ApiError {
    let mut api_error = match err.kind() {
        TagServiceErrorKind::Invalid => ApiError::bad_request(detail),
        TagServiceErrorKind::NotFound => ApiError::not_found(detail),
        TagServiceErrorKind::Conflict => ApiError::conflict(detail),
        TagServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        TagServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::TagListItemResponse;
    use axum::{extract::Path, response::IntoResponse};
    use chrono::Utc;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn list_tags_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_list_items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(TagListItemResponse {
                tag_public_id: Uuid::new_v4(),
                tag_key: "favorites".to_string(),
                display_name: "Favorites".to_string(),
                updated_at: Utc::now(),
            });
        let state = indexer_test_state(indexers)?;

        let Json(response) = list_tags(State(state)).await?;
        assert_eq!(response.tags.len(), 1);
        assert_eq!(response.tags[0].tag_key, "favorites");
        Ok(())
    }

    #[tokio::test]
    async fn list_tags_maps_storage_errors() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(TagServiceError::new(TagServiceErrorKind::Storage));
        let state = indexer_test_state(indexers)?;

        let err = list_tags(State(state))
            .await
            .expect_err("storage error should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_LIST_FAILED));
        Ok(())
    }

    #[tokio::test]
    async fn create_tag_trims_values_and_returns_payload() -> Result<(), ApiError> {
        let tag_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_result
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(Ok(tag_public_id));
        let state = indexer_test_state(indexers.clone())?;

        let request = TagCreateRequest {
            tag_key: " favorites ".to_string(),
            display_name: " Favorites ".to_string(),
        };

        let (status, Json(response)) = create_tag(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.tag_public_id, tag_public_id);
        assert_eq!(response.tag_key.as_deref(), Some("favorites"));
        assert_eq!(response.display_name, "Favorites");

        let calls = indexers
            .tag_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, "favorites");
        assert_eq!(calls[0].2, "Favorites");
        Ok(())
    }

    #[tokio::test]
    async fn create_tag_invalid_maps_bad_request() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(TagServiceError::new(TagServiceErrorKind::Invalid).with_code("tag_key_empty"));
        let state = indexer_test_state(indexers)?;

        let request = TagCreateRequest {
            tag_key: "favorites".to_string(),
            display_name: "Name".to_string(),
        };

        let err = create_tag(State(state), Json(request))
            .await
            .expect_err("invalid tag input should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_CREATE_FAILED));
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }

    #[tokio::test]
    async fn create_tag_requires_non_blank_fields() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;

        let err = create_tag(
            State(state.clone()),
            Json(TagCreateRequest {
                tag_key: "   ".to_string(),
                display_name: "Favorites".to_string(),
            }),
        )
        .await
        .expect_err("blank tag key should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_KEY_REQUIRED));

        let err = create_tag(
            State(state),
            Json(TagCreateRequest {
                tag_key: "favorites".to_string(),
                display_name: "   ".to_string(),
            }),
        )
        .await
        .expect_err("blank display name should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_DISPLAY_NAME_REQUIRED));
        Ok(())
    }

    #[tokio::test]
    async fn update_tag_not_found_maps_not_found() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(
                TagServiceError::new(TagServiceErrorKind::NotFound).with_code("tag_not_found"),
            );
        let state = indexer_test_state(indexers)?;

        let request = TagUpdateRequest {
            tag_public_id: Some(Uuid::new_v4()),
            tag_key: None,
            display_name: "Updated".to_string(),
        };

        let err = update_tag(State(state), Json(request))
            .await
            .expect_err("missing tag should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn update_tag_requires_identifier() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;
        let request = TagUpdateRequest {
            tag_public_id: None,
            tag_key: Some("   ".to_string()),
            display_name: "Updated".to_string(),
        };

        let err = update_tag(State(state), Json(request))
            .await
            .expect_err("missing tag reference should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_REFERENCE_REQUIRED));
        Ok(())
    }

    #[tokio::test]
    async fn delete_tag_requires_identifier() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;
        let request = TagDeleteRequest {
            tag_public_id: None,
            tag_key: Some("   ".to_string()),
        };

        let err = delete_tag(State(state), Json(request))
            .await
            .expect_err("missing tag reference should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(TAG_REFERENCE_REQUIRED));
        Ok(())
    }

    #[tokio::test]
    async fn delete_tag_by_key_trims_path_value() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;

        let status = delete_tag_by_key(State(state), Path("  quality  ".to_string())).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);
        Ok(())
    }

    #[tokio::test]
    async fn delete_tag_by_key_reports_distinct_operation_context() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers
            .tag_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(TagServiceError::new(TagServiceErrorKind::Storage));
        let state = indexer_test_state(indexers)?;

        let err = delete_tag_by_key(State(state), Path("quality".to_string()))
            .await
            .expect_err("storage error should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(
            context
                .iter()
                .any(|field| field.name == "operation" && field.value == "tag_delete_by_key")
        );
        Ok(())
    }
}
