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
use crate::http::handlers::indexers::normalization::trim_and_filter_empty;
use crate::models::{
    MediaDomainMappingDeleteRequest, MediaDomainMappingUpsertRequest,
    TrackerCategoryMappingDeleteRequest, TrackerCategoryMappingUpsertRequest,
};

const TRACKER_MAPPING_UPSERT_FAILED: &str = "failed to upsert tracker category mapping";
const TRACKER_MAPPING_DELETE_FAILED: &str = "failed to delete tracker category mapping";
const MEDIA_DOMAIN_MAPPING_UPSERT_FAILED: &str = "failed to upsert media domain mapping";
const MEDIA_DOMAIN_MAPPING_DELETE_FAILED: &str = "failed to delete media domain mapping";

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
    let media_domain_key = request.media_domain_key.trim();
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
    let media_domain_key = request.media_domain_key.trim();
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
