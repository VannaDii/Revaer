//! Category mapping endpoints for indexers.
//!
//! # Design
//! - Delegate category mapping operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.
//! - Explain mapping layers: tracker categories map to Torznab categories and media domains to
//!   support consistent downstream filtering.

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};

use crate::app::indexers::{
    CategoryMappingServiceError, CategoryMappingServiceErrorKind,
    TrackerCategoryMappingDeleteParams, TrackerCategoryMappingUpsertParams,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{
    MediaDomainMappingDeleteRequest, MediaDomainMappingUpsertRequest,
    TrackerCategoryMappingDeleteRequest, TrackerCategoryMappingUpsertRequest,
};

const TRACKER_MAPPING_UPSERT_FAILED: &str = "failed to upsert tracker category mapping";
const TRACKER_MAPPING_DELETE_FAILED: &str = "failed to delete tracker category mapping";
const MEDIA_DOMAIN_MAPPING_UPSERT_FAILED: &str = "failed to upsert media domain mapping";
const MEDIA_DOMAIN_MAPPING_DELETE_FAILED: &str = "failed to delete media domain mapping";
const MEDIA_DOMAIN_KEY_REQUIRED: &str = "media_domain_key is required";

pub(crate) async fn upsert_tracker_category_mapping(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TrackerCategoryMappingUpsertRequest>,
) -> Result<StatusCode, ApiError> {
    let upstream_slug = trim_and_filter_empty(request.indexer_definition_upstream_slug.as_deref());
    let torznab_instance_public_id = request.torznab_instance_public_id;
    let indexer_instance_public_id = request.indexer_instance_public_id;
    let media_domain_key = trim_and_filter_empty(request.media_domain_key.as_deref());
    state
        .indexers
        .tracker_category_mapping_upsert(TrackerCategoryMappingUpsertParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            torznab_instance_public_id,
            indexer_definition_upstream_slug: upstream_slug,
            indexer_instance_public_id,
            tracker_category: request.tracker_category,
            tracker_subcategory: request.tracker_subcategory,
            torznab_cat_id: request.torznab_cat_id,
            media_domain_key,
        })
        .await
        .map_err(|err| {
            map_category_mapping_error(
                "tracker_category_mapping_upsert",
                TRACKER_MAPPING_UPSERT_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_tracker_category_mapping(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TrackerCategoryMappingDeleteRequest>,
) -> Result<StatusCode, ApiError> {
    let upstream_slug = trim_and_filter_empty(request.indexer_definition_upstream_slug.as_deref());
    let torznab_instance_public_id = request.torznab_instance_public_id;
    let indexer_instance_public_id = request.indexer_instance_public_id;
    state
        .indexers
        .tracker_category_mapping_delete(TrackerCategoryMappingDeleteParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            torznab_instance_public_id,
            indexer_definition_upstream_slug: upstream_slug,
            indexer_instance_public_id,
            tracker_category: request.tracker_category,
            tracker_subcategory: request.tracker_subcategory,
        })
        .await
        .map_err(|err| {
            map_category_mapping_error(
                "tracker_category_mapping_delete",
                TRACKER_MAPPING_DELETE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn upsert_media_domain_mapping(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MediaDomainMappingUpsertRequest>,
) -> Result<StatusCode, ApiError> {
    let media_domain_key =
        normalize_required_str_field(&request.media_domain_key, MEDIA_DOMAIN_KEY_REQUIRED)?;
    state
        .indexers
        .media_domain_mapping_upsert(
            SYSTEM_ACTOR_PUBLIC_ID,
            media_domain_key,
            request.torznab_cat_id,
            request.is_primary,
        )
        .await
        .map_err(|err| {
            map_category_mapping_error(
                "media_domain_mapping_upsert",
                MEDIA_DOMAIN_MAPPING_UPSERT_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_media_domain_mapping(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<MediaDomainMappingDeleteRequest>,
) -> Result<StatusCode, ApiError> {
    let media_domain_key =
        normalize_required_str_field(&request.media_domain_key, MEDIA_DOMAIN_KEY_REQUIRED)?;
    state
        .indexers
        .media_domain_mapping_delete(
            SYSTEM_ACTOR_PUBLIC_ID,
            media_domain_key,
            request.torznab_cat_id,
        )
        .await
        .map_err(|err| {
            map_category_mapping_error(
                "media_domain_mapping_delete",
                MEDIA_DOMAIN_MAPPING_DELETE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_category_mapping_error(
    operation: &'static str,
    detail: &'static str,
    err: &CategoryMappingServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        CategoryMappingServiceErrorKind::Invalid => ApiError::bad_request(detail),
        CategoryMappingServiceErrorKind::NotFound => ApiError::not_found(detail),
        CategoryMappingServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        CategoryMappingServiceErrorKind::Storage => ApiError::internal(detail),
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
    use axum::response::IntoResponse;
    use std::sync::Arc;

    #[tokio::test]
    async fn upsert_media_domain_mapping_rejects_blank_key() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;
        let request = MediaDomainMappingUpsertRequest {
            media_domain_key: "   ".into(),
            torznab_cat_id: 2000,
            is_primary: Some(true),
        };

        let err = upsert_media_domain_mapping(State(state), Json(request))
            .await
            .expect_err("blank media_domain_key should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(MEDIA_DOMAIN_KEY_REQUIRED));
        Ok(())
    }

    #[tokio::test]
    async fn delete_media_domain_mapping_rejects_blank_key() -> Result<(), ApiError> {
        let state = indexer_test_state(Arc::new(RecordingIndexers::default()))?;
        let request = MediaDomainMappingDeleteRequest {
            media_domain_key: "\n".into(),
            torznab_cat_id: 2000,
        };

        let err = delete_media_domain_mapping(State(state), Json(request))
            .await
            .expect_err("blank media_domain_key should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(MEDIA_DOMAIN_KEY_REQUIRED));
        Ok(())
    }

    #[tokio::test]
    async fn upsert_tracker_category_mapping_trims_optional_values() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;

        let status = upsert_tracker_category_mapping(
            State(state),
            Json(TrackerCategoryMappingUpsertRequest {
                torznab_instance_public_id: Some(uuid::Uuid::new_v4()),
                indexer_definition_upstream_slug: Some("  prowlarr ".into()),
                indexer_instance_public_id: Some(uuid::Uuid::new_v4()),
                tracker_category: 5000,
                tracker_subcategory: Some(42),
                torznab_cat_id: 5030,
                media_domain_key: Some("  tv ".into()),
            }),
        )
        .await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        let calls = indexers
            .tracker_category_mapping_upsert_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.as_deref(), Some("prowlarr"));
        assert_eq!(calls[0].6.as_deref(), Some("tv"));
        Ok(())
    }

    #[tokio::test]
    async fn delete_tracker_category_mapping_trims_optional_slug() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;

        let status = delete_tracker_category_mapping(
            State(state),
            Json(TrackerCategoryMappingDeleteRequest {
                torznab_instance_public_id: None,
                indexer_definition_upstream_slug: Some("  cardigann ".into()),
                indexer_instance_public_id: None,
                tracker_category: 1000,
                tracker_subcategory: None,
            }),
        )
        .await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        let calls = indexers
            .tracker_category_mapping_delete_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1.as_deref(), Some("cardigann"));
        Ok(())
    }

    #[tokio::test]
    async fn upsert_media_domain_mapping_trims_key_and_records_primary() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;

        let status = upsert_media_domain_mapping(
            State(state),
            Json(MediaDomainMappingUpsertRequest {
                media_domain_key: "  movies ".into(),
                torznab_cat_id: 2000,
                is_primary: Some(true),
            }),
        )
        .await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        let calls = indexers
            .media_domain_mapping_upsert_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, "movies".into(), 2000, Some(true))]
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_media_domain_mapping_trims_key_and_records_category() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;

        let status = delete_media_domain_mapping(
            State(state),
            Json(MediaDomainMappingDeleteRequest {
                media_domain_key: "  books ".into(),
                torznab_cat_id: 7000,
            }),
        )
        .await?;

        assert_eq!(status, StatusCode::NO_CONTENT);
        let calls = indexers
            .media_domain_mapping_delete_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, "books".into(), 7000)]
        );
        Ok(())
    }
}
