//! Indexer instance management endpoints.
//!
//! # Design
//! - Delegate all instance lifecycle operations to the injected indexer facade.
//! - Keep responses minimal and deterministic for CLI/automation use.
//! - Surface RFC9457 problem details with stable, constant messages.

use std::sync::Arc;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use uuid::Uuid;

use crate::app::indexers::{
    IndexerCfStateResetParams, IndexerInstanceFieldError, IndexerInstanceFieldErrorKind,
    IndexerInstanceFieldValueParams, IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
    IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{
    IndexerCfStateResetRequest, IndexerCfStateResponse, IndexerInstanceCreateRequest,
    IndexerInstanceFieldSecretBindRequest, IndexerInstanceFieldValueRequest,
    IndexerInstanceListResponse, IndexerInstanceMediaDomainsRequest, IndexerInstanceResponse,
    IndexerInstanceTagsRequest, IndexerInstanceTestFinalizeRequest,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerInstanceUpdateRequest,
};

const INSTANCE_LIST_FAILED: &str = "failed to fetch indexer instances";
const INSTANCE_CREATE_FAILED: &str = "failed to create indexer instance";
const INSTANCE_UPDATE_FAILED: &str = "failed to update indexer instance";
const INSTANCE_MEDIA_DOMAINS_FAILED: &str = "failed to set media domains";
const INSTANCE_TAGS_FAILED: &str = "failed to set tags";
const INSTANCE_FIELD_VALUE_FAILED: &str = "failed to set field value";
const INSTANCE_FIELD_SECRET_FAILED: &str = "failed to bind field secret";
const INSTANCE_CF_STATE_RESET_FAILED: &str = "failed to reset cf state";
const INSTANCE_CF_STATE_GET_FAILED: &str = "failed to fetch cf state";
const INSTANCE_TEST_PREPARE_FAILED: &str = "failed to prepare instance test";
const INSTANCE_TEST_FINALIZE_FAILED: &str = "failed to finalize instance test";
const INSTANCE_TAG_KEYS_TOO_LARGE: &str = "tag_keys exceeds maximum size";
const INSTANCE_TAG_KEY_TOO_LARGE: &str = "tag_key exceeds maximum size";
const INSTANCE_TAG_KEYS_MAX_LEN: usize = 1024;
const INSTANCE_TAG_KEY_MAX_BYTES: usize = 1024;

pub(crate) async fn create_indexer_instance(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<IndexerInstanceCreateRequest>,
) -> Result<(StatusCode, Json<IndexerInstanceResponse>), ApiError> {
    let display_name = request.display_name.trim();
    let indexer_definition_upstream_slug = request.indexer_definition_upstream_slug.trim();
    let trust_tier_key = trim_and_filter_empty(request.trust_tier_key.as_deref());
    let routing_policy_public_id = request.routing_policy_public_id;

    let indexer_instance_public_id = state
        .indexers
        .indexer_instance_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            indexer_definition_upstream_slug,
            display_name,
            request.priority,
            trust_tier_key,
            routing_policy_public_id,
        )
        .await
        .map_err(|err| {
            map_instance_error("indexer_instance_create", INSTANCE_CREATE_FAILED, &err)
        })?;

    Ok((
        StatusCode::CREATED,
        Json(IndexerInstanceResponse {
            indexer_instance_public_id,
        }),
    ))
}

pub(crate) async fn list_indexer_instances(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IndexerInstanceListResponse>, ApiError> {
    let indexer_instances = state
        .indexers
        .indexer_instance_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| map_instance_error("indexer_instance_list", INSTANCE_LIST_FAILED, &err))?;

    Ok(Json(IndexerInstanceListResponse { indexer_instances }))
}

pub(crate) async fn update_indexer_instance(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceUpdateRequest>,
) -> Result<Json<IndexerInstanceResponse>, ApiError> {
    let display_name = trim_and_filter_empty(request.display_name.as_deref());
    let trust_tier_key = trim_and_filter_empty(request.trust_tier_key.as_deref());
    let routing_policy_public_id = request.routing_policy_public_id;

    let params = IndexerInstanceUpdateParams {
        actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
        indexer_instance_public_id,
        display_name,
        priority: request.priority,
        trust_tier_key,
        routing_policy_public_id,
        is_enabled: request.is_enabled,
        enable_rss: request.enable_rss,
        enable_automatic_search: request.enable_automatic_search,
        enable_interactive_search: request.enable_interactive_search,
    };

    let updated = state
        .indexers
        .indexer_instance_update(params)
        .await
        .map_err(|err| {
            map_instance_error("indexer_instance_update", INSTANCE_UPDATE_FAILED, &err)
        })?;

    Ok(Json(IndexerInstanceResponse {
        indexer_instance_public_id: updated,
    }))
}

pub(crate) async fn set_indexer_instance_media_domains(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceMediaDomainsRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .indexer_instance_set_media_domains(
            SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            &request.media_domain_keys,
        )
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_instance_set_media_domains",
                INSTANCE_MEDIA_DOMAINS_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_indexer_instance_tags(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceTagsRequest>,
) -> Result<StatusCode, ApiError> {
    let normalized_tag_keys = request.tag_keys.map(normalize_tag_keys).transpose()?;
    let tag_public_ids = request.tag_public_ids.as_deref();
    let tag_keys = normalized_tag_keys.as_deref();
    state
        .indexers
        .indexer_instance_set_tags(
            SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            tag_public_ids,
            tag_keys,
        )
        .await
        .map_err(|err| {
            map_instance_error("indexer_instance_set_tags", INSTANCE_TAGS_FAILED, &err)
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn normalize_tag_keys(keys: Vec<String>) -> Result<Vec<String>, ApiError> {
    if keys.len() > INSTANCE_TAG_KEYS_MAX_LEN {
        let mut error = ApiError::bad_request(INSTANCE_TAG_KEYS_TOO_LARGE);
        error = error.with_context_field("max_len", INSTANCE_TAG_KEYS_MAX_LEN.to_string());
        return Err(error);
    }

    let mut normalized_len = 0_usize;
    for key in &keys {
        let trimmed = key.trim();
        if trimmed.len() > INSTANCE_TAG_KEY_MAX_BYTES {
            let mut error = ApiError::bad_request(INSTANCE_TAG_KEY_TOO_LARGE);
            error = error.with_context_field("max_len", INSTANCE_TAG_KEY_MAX_BYTES.to_string());
            return Err(error);
        }
        if !trimmed.is_empty() {
            normalized_len = normalized_len.saturating_add(1);
        }
    }

    let mut normalized = Vec::with_capacity(normalized_len);
    for key in keys {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            normalized.push(trimmed.to_string());
        }
    }
    Ok(normalized)
}

pub(crate) async fn set_indexer_instance_field_value(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceFieldValueRequest>,
) -> Result<StatusCode, ApiError> {
    let field_name = request.field_name.trim();
    let value_plain = trim_and_filter_empty(request.value_plain.as_deref());
    let value_decimal = trim_and_filter_empty(request.value_decimal.as_deref());

    let params = IndexerInstanceFieldValueParams {
        actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
        indexer_instance_public_id,
        field_name,
        value_plain,
        value_int: request.value_int,
        value_decimal,
        value_bool: request.value_bool,
    };

    state
        .indexers
        .indexer_instance_field_set_value(params)
        .await
        .map_err(|err| {
            map_instance_field_error(
                "indexer_instance_field_set_value",
                INSTANCE_FIELD_VALUE_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn bind_indexer_instance_field_secret(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceFieldSecretBindRequest>,
) -> Result<StatusCode, ApiError> {
    let field_name = request.field_name.trim();

    state
        .indexers
        .indexer_instance_field_bind_secret(
            SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            field_name,
            request.secret_public_id,
        )
        .await
        .map_err(|err| {
            map_instance_field_error(
                "indexer_instance_field_bind_secret",
                INSTANCE_FIELD_SECRET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn reset_indexer_instance_cf_state(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerCfStateResetRequest>,
) -> Result<StatusCode, ApiError> {
    let reason = normalize_required_str_field(&request.reason, "reason is required")?;

    let params = IndexerCfStateResetParams {
        actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
        indexer_instance_public_id,
        reason,
    };

    state
        .indexers
        .indexer_cf_state_reset(params)
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_cf_state_reset",
                INSTANCE_CF_STATE_RESET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn prepare_indexer_instance_test(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
) -> Result<Json<IndexerInstanceTestPrepareResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_instance_test_prepare(SYSTEM_ACTOR_PUBLIC_ID, indexer_instance_public_id)
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_instance_test_prepare",
                INSTANCE_TEST_PREPARE_FAILED,
                &err,
            )
        })?;

    Ok(Json(response))
}

pub(crate) async fn finalize_indexer_instance_test(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerInstanceTestFinalizeRequest>,
) -> Result<Json<IndexerInstanceTestFinalizeResponse>, ApiError> {
    let error_class = trim_and_filter_empty(request.error_class.as_deref());
    let error_code = trim_and_filter_empty(request.error_code.as_deref());
    let detail = trim_and_filter_empty(request.detail.as_deref());

    let params = IndexerInstanceTestFinalizeParams {
        actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
        indexer_instance_public_id,
        ok: request.ok,
        error_class,
        error_code,
        detail,
        result_count: request.result_count,
    };

    let response = state
        .indexers
        .indexer_instance_test_finalize(params)
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_instance_test_finalize",
                INSTANCE_TEST_FINALIZE_FAILED,
                &err,
            )
        })?;

    Ok(Json(response))
}

pub(crate) async fn get_indexer_instance_cf_state(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
) -> Result<Json<IndexerCfStateResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_cf_state_get(SYSTEM_ACTOR_PUBLIC_ID, indexer_instance_public_id)
        .await
        .map_err(|err| {
            map_instance_error("indexer_cf_state_get", INSTANCE_CF_STATE_GET_FAILED, &err)
        })?;

    Ok(Json(response))
}

pub(super) fn map_instance_error(
    operation: &'static str,
    detail: &'static str,
    err: &IndexerInstanceServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        IndexerInstanceServiceErrorKind::Invalid => ApiError::bad_request(detail),
        IndexerInstanceServiceErrorKind::NotFound => ApiError::not_found(detail),
        IndexerInstanceServiceErrorKind::Conflict => ApiError::conflict(detail),
        IndexerInstanceServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        IndexerInstanceServiceErrorKind::Storage => ApiError::internal(detail),
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

pub(super) fn map_instance_field_error(
    operation: &'static str,
    detail: &'static str,
    err: &IndexerInstanceFieldError,
) -> ApiError {
    let mut api_error = match err.kind() {
        IndexerInstanceFieldErrorKind::Invalid => ApiError::bad_request(detail),
        IndexerInstanceFieldErrorKind::NotFound => ApiError::not_found(detail),
        IndexerInstanceFieldErrorKind::Conflict => ApiError::conflict(detail),
        IndexerInstanceFieldErrorKind::Unauthorized => ApiError::unauthorized(detail),
        IndexerInstanceFieldErrorKind::Storage => ApiError::internal(detail),
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
    use crate::app::indexers::{
        CategoryMappingServiceError, CategoryMappingServiceErrorKind, IndexerCfStateResetParams,
        IndexerDefinitionServiceError, IndexerFacade, IndexerInstanceFieldError,
        IndexerInstanceFieldErrorKind, IndexerInstanceFieldValueParams,
        IndexerInstanceServiceError, IndexerInstanceTestFinalizeParams,
        IndexerInstanceUpdateParams, RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind,
        RoutingPolicyServiceError, RoutingPolicyServiceErrorKind, SearchProfileServiceError,
        SearchProfileServiceErrorKind, SecretServiceError, SecretServiceErrorKind, TagServiceError,
        TagServiceErrorKind, TorznabInstanceCredentials, TorznabInstanceServiceError,
        TorznabInstanceServiceErrorKind,
    };
    use crate::http::handlers::indexers::test_support::{indexer_test_state, parse_problem};
    use crate::models::{IndexerCfStateResponse, IndexerDefinitionResponse};
    use async_trait::async_trait;
    use axum::response::IntoResponse;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingIndexers {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl IndexerFacade for RecordingIndexers {
        async fn indexer_definition_list(
            &self,
            _actor_user_public_id: Uuid,
        ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
            Ok(Vec::new())
        }

        async fn tag_create(
            &self,
            _actor_user_public_id: Uuid,
            _tag_key: &str,
            _display_name: &str,
        ) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_update(
            &self,
            _actor_user_public_id: Uuid,
            _tag_public_id: Option<Uuid>,
            _tag_key: Option<&str>,
            _display_name: &str,
        ) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_delete(
            &self,
            _actor_user_public_id: Uuid,
            _tag_public_id: Option<Uuid>,
            _tag_key: Option<&str>,
        ) -> Result<(), TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn routing_policy_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _mode: &str,
        ) -> Result<Uuid, RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_set_param(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _param_key: &str,
            _value_plain: Option<&str>,
            _value_int: Option<i32>,
            _value_bool: Option<bool>,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _param_key: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _rpm: i32,
            _burst: i32,
            _concurrent: i32,
        ) -> Result<Uuid, RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_update(
            &self,
            _actor_user_public_id: Uuid,
            _rate_limit_policy_public_id: Uuid,
            _display_name: Option<&str>,
            _rpm: Option<i32>,
            _burst: Option<i32>,
            _concurrent: Option<i32>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_soft_delete(
            &self,
            _actor_user_public_id: Uuid,
            _rate_limit_policy_public_id: Uuid,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _is_default: Option<bool>,
            _page_size: Option<i32>,
            _default_media_domain_key: Option<&str>,
            _user_public_id: Option<Uuid>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_update(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _display_name: Option<&str>,
            _page_size: Option<i32>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_default(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _page_size: Option<i32>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_default_domain(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _default_media_domain_key: Option<&str>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_set_domain_allowlist(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_add_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_remove_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_indexer_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_indexer_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn search_profile_tag_prefer(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Err(SearchProfileServiceError::new(
                SearchProfileServiceErrorKind::Storage,
            ))
        }

        async fn tracker_category_mapping_upsert(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn tracker_category_mapping_delete(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_upsert(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
            _is_primary: Option<bool>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_delete(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_create(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _display_name: &str,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_rotate_key(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_enable_disable(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
            _is_enabled: bool,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_soft_delete(
            &self,
            _actor_user_public_id: Uuid,
            _torznab_instance_public_id: Uuid,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_create(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_definition_upstream_slug: &str,
            _display_name: &str,
            _priority: Option<i32>,
            _trust_tier_key: Option<&str>,
            _routing_policy_public_id: Option<Uuid>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("create".to_string());
            Ok(Uuid::new_v4())
        }

        async fn indexer_instance_update(
            &self,
            params: IndexerInstanceUpdateParams<'_>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("update".to_string());
            Ok(params.indexer_instance_public_id)
        }

        async fn indexer_instance_set_media_domains(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), IndexerInstanceServiceError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("media_domains".to_string());
            Ok(())
        }

        async fn indexer_instance_set_tags(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), IndexerInstanceServiceError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("tags".to_string());
            Ok(())
        }

        async fn indexer_instance_field_set_value(
            &self,
            _params: IndexerInstanceFieldValueParams<'_>,
        ) -> Result<(), IndexerInstanceFieldError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("field_value".to_string());
            Ok(())
        }

        async fn indexer_instance_field_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _field_name: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), IndexerInstanceFieldError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("field_secret".to_string());
            Ok(())
        }

        async fn indexer_cf_state_reset(
            &self,
            _params: IndexerCfStateResetParams<'_>,
        ) -> Result<(), IndexerInstanceServiceError> {
            self.calls
                .lock()
                .expect("lock poisoned")
                .push("cf_reset".to_string());
            Ok(())
        }

        async fn indexer_cf_state_get(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
            Ok(IndexerCfStateResponse {
                state: "clear".to_string(),
                last_changed_at: chrono::Utc::now(),
                cf_session_expires_at: None,
                cooldown_until: None,
                backoff_seconds: None,
                consecutive_failures: 0,
                last_error_class: None,
            })
        }

        async fn indexer_instance_test_prepare(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
            Ok(IndexerInstanceTestPrepareResponse {
                can_execute: true,
                error_class: None,
                error_code: None,
                detail: None,
                engine: "torznab".to_string(),
                routing_policy_public_id: None,
                connect_timeout_ms: 1000,
                read_timeout_ms: 1000,
                field_names: None,
                field_types: None,
                value_plain: None,
                value_int: None,
                value_decimal: None,
                value_bool: None,
                secret_public_ids: None,
            })
        }

        async fn indexer_instance_test_finalize(
            &self,
            _params: IndexerInstanceTestFinalizeParams<'_>,
        ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
            Ok(IndexerInstanceTestFinalizeResponse {
                ok: true,
                error_class: None,
                error_code: None,
                detail: None,
                result_count: Some(1),
            })
        }

        async fn secret_create(
            &self,
            _actor_user_public_id: Uuid,
            _secret_type: &str,
            _secret_value: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_rotate(
            &self,
            _actor_user_public_id: Uuid,
            _secret_public_id: Uuid,
            _secret_value: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_revoke(
            &self,
            _actor_user_public_id: Uuid,
            _secret_public_id: Uuid,
        ) -> Result<(), SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }
    }

    #[tokio::test]
    async fn create_instance_returns_created() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers).unwrap();
        let request = IndexerInstanceCreateRequest {
            indexer_definition_upstream_slug: "example".into(),
            display_name: "Example".into(),
            priority: Some(10),
            trust_tier_key: Some("public".into()),
            routing_policy_public_id: None,
        };

        let response = create_indexer_instance(State(state), Json(request))
            .await
            .unwrap()
            .0;

        assert_eq!(response, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn set_indexer_instance_tags_rejects_excessive_key_count() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;
        let request = IndexerInstanceTagsRequest {
            tag_public_ids: None,
            tag_keys: Some(vec!["tag".to_string(); INSTANCE_TAG_KEYS_MAX_LEN + 1]),
        };

        let err = set_indexer_instance_tags(State(state), Path(Uuid::new_v4()), Json(request))
            .await
            .expect_err("excessive tag key count should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(INSTANCE_TAG_KEYS_TOO_LARGE));
        Ok(())
    }

    #[tokio::test]
    async fn set_indexer_instance_tags_rejects_oversized_key() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;
        let request = IndexerInstanceTagsRequest {
            tag_public_ids: None,
            tag_keys: Some(vec!["x".repeat(INSTANCE_TAG_KEY_MAX_BYTES + 1)]),
        };

        let err = set_indexer_instance_tags(State(state), Path(Uuid::new_v4()), Json(request))
            .await
            .expect_err("oversized tag key should fail");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(INSTANCE_TAG_KEY_TOO_LARGE));
        Ok(())
    }

    #[tokio::test]
    async fn set_field_secret_propagates_conflict() {
        let err = IndexerInstanceFieldError::new(IndexerInstanceFieldErrorKind::Conflict)
            .with_code("field_not_secret")
            .with_sqlstate("P0001");
        let result = map_instance_field_error("test", INSTANCE_FIELD_SECRET_FAILED, &err);
        assert_eq!(result.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn get_cf_state_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;
        let indexer_instance_public_id = Uuid::new_v4();

        let Json(payload) =
            get_indexer_instance_cf_state(State(state), Path(indexer_instance_public_id)).await?;
        assert_eq!(payload.state, "clear");
        Ok(())
    }

    #[tokio::test]
    async fn prepare_test_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;
        let indexer_instance_public_id = Uuid::new_v4();

        let Json(payload) =
            prepare_indexer_instance_test(State(state), Path(indexer_instance_public_id)).await?;
        assert!(payload.can_execute);
        assert_eq!(payload.engine, "torznab");
        Ok(())
    }

    #[tokio::test]
    async fn finalize_test_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers)?;
        let indexer_instance_public_id = Uuid::new_v4();
        let request = IndexerInstanceTestFinalizeRequest {
            ok: true,
            error_class: None,
            error_code: None,
            detail: None,
            result_count: Some(1),
        };

        let Json(payload) = finalize_indexer_instance_test(
            State(state),
            Path(indexer_instance_public_id),
            Json(request),
        )
        .await?;
        assert!(payload.ok);
        Ok(())
    }
}
