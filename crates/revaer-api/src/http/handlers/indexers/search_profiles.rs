//! Search profile endpoints for indexers.
//!
//! # Design
//! - Delegate search profile operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::{mem, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::app::indexers::{SearchProfileServiceError, SearchProfileServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::allocation::{checked_vec_capacity, ensure_allocation_safe};
use crate::models::{
    SearchProfileCreateRequest, SearchProfileDefaultDomainRequest, SearchProfileDefaultRequest,
    SearchProfileDomainAllowlistRequest, SearchProfileIndexerSetRequest,
    SearchProfilePolicySetRequest, SearchProfileResponse, SearchProfileTagSetRequest,
    SearchProfileUpdateRequest,
};

const SEARCH_PROFILE_CREATE_FAILED: &str = "failed to create search profile";
const SEARCH_PROFILE_UPDATE_FAILED: &str = "failed to update search profile";
const SEARCH_PROFILE_SET_DEFAULT_FAILED: &str = "failed to set search profile default";
const SEARCH_PROFILE_SET_DEFAULT_DOMAIN_FAILED: &str =
    "failed to set search profile default media domain";
const SEARCH_PROFILE_SET_DOMAIN_ALLOWLIST_FAILED: &str =
    "failed to set search profile domain allowlist";
const SEARCH_PROFILE_ADD_POLICY_SET_FAILED: &str = "failed to add search profile policy set";
const SEARCH_PROFILE_REMOVE_POLICY_SET_FAILED: &str = "failed to remove search profile policy set";
const SEARCH_PROFILE_INDEXER_ALLOW_FAILED: &str = "failed to allow search profile indexers";
const SEARCH_PROFILE_INDEXER_BLOCK_FAILED: &str = "failed to block search profile indexers";
const SEARCH_PROFILE_TAG_ALLOW_FAILED: &str = "failed to allow search profile tags";
const SEARCH_PROFILE_TAG_BLOCK_FAILED: &str = "failed to block search profile tags";
const SEARCH_PROFILE_TAG_PREFER_FAILED: &str = "failed to prefer search profile tags";
const SEARCH_PROFILE_DOMAIN_KEYS_TOO_LARGE: &str = "media_domain_keys exceeds maximum size";
const SEARCH_PROFILE_DOMAIN_KEY_TOO_LARGE: &str = "media_domain_key exceeds maximum size";
const SEARCH_PROFILE_TAG_KEYS_TOO_LARGE: &str = "tag_keys exceeds maximum size";
const SEARCH_PROFILE_TAG_KEY_TOO_LARGE: &str = "tag_key exceeds maximum size";
const SEARCH_PROFILE_DOMAIN_KEYS_MAX_LEN: usize = 2048;
const SEARCH_PROFILE_DOMAIN_KEY_MAX_BYTES: usize = 4096;
const SEARCH_PROFILE_TAG_KEYS_MAX_LEN: usize = 1024;
const SEARCH_PROFILE_TAG_KEY_MAX_BYTES: usize = 1024;

pub(crate) async fn create_search_profile(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileCreateRequest>,
) -> Result<(StatusCode, Json<SearchProfileResponse>), ApiError> {
    let display_name = request.display_name.trim();
    let default_media_domain_key = request.default_media_domain_key.as_deref().map(str::trim);
    let search_profile_public_id = state
        .indexers
        .search_profile_create(
            SYSTEM_ACTOR_PUBLIC_ID,
            display_name,
            request.is_default,
            request.page_size,
            default_media_domain_key,
            request.user_public_id,
        )
        .await
        .map_err(|err| {
            map_search_profile_error("search_profile_create", SEARCH_PROFILE_CREATE_FAILED, &err)
        })?;

    Ok((
        StatusCode::CREATED,
        Json(SearchProfileResponse {
            search_profile_public_id,
        }),
    ))
}

pub(crate) async fn update_search_profile(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileUpdateRequest>,
) -> Result<Json<SearchProfileResponse>, ApiError> {
    let display_name = request.display_name.as_deref().map(str::trim);
    let search_profile_public_id = state
        .indexers
        .search_profile_update(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            display_name,
            request.page_size,
        )
        .await
        .map_err(|err| {
            map_search_profile_error("search_profile_update", SEARCH_PROFILE_UPDATE_FAILED, &err)
        })?;

    Ok(Json(SearchProfileResponse {
        search_profile_public_id,
    }))
}

pub(crate) async fn set_search_profile_default(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileDefaultRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .search_profile_set_default(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.page_size,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_set_default",
                SEARCH_PROFILE_SET_DEFAULT_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_default_domain(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileDefaultDomainRequest>,
) -> Result<StatusCode, ApiError> {
    let default_media_domain_key = request.default_media_domain_key.as_deref().map(str::trim);
    state
        .indexers
        .search_profile_set_default_domain(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            default_media_domain_key,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_set_default_domain",
                SEARCH_PROFILE_SET_DEFAULT_DOMAIN_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_domain_allowlist(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileDomainAllowlistRequest>,
) -> Result<StatusCode, ApiError> {
    let requested_bytes = calculate_domain_key_bytes(&request.media_domain_keys)?;
    ensure_allocation_safe(requested_bytes)?;
    let capacity = request
        .media_domain_keys
        .len()
        .min(SEARCH_PROFILE_DOMAIN_KEYS_MAX_LEN);
    let mut media_domain_keys = checked_vec_capacity::<String>(capacity)?;
    for key in request.media_domain_keys {
        media_domain_keys.push(key.trim().to_string());
    }
    state
        .indexers
        .search_profile_set_domain_allowlist(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            &media_domain_keys,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_set_domain_allowlist",
                SEARCH_PROFILE_SET_DOMAIN_ALLOWLIST_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn add_search_profile_policy_set(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfilePolicySetRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .search_profile_add_policy_set(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.policy_set_public_id,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_add_policy_set",
                SEARCH_PROFILE_ADD_POLICY_SET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn remove_search_profile_policy_set(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfilePolicySetRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .search_profile_remove_policy_set(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.policy_set_public_id,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_remove_policy_set",
                SEARCH_PROFILE_REMOVE_POLICY_SET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_indexer_allow(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileIndexerSetRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .search_profile_indexer_allow(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            &request.indexer_instance_public_ids,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_indexer_allow",
                SEARCH_PROFILE_INDEXER_ALLOW_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_indexer_block(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileIndexerSetRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .search_profile_indexer_block(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            &request.indexer_instance_public_ids,
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_indexer_block",
                SEARCH_PROFILE_INDEXER_BLOCK_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_tag_allow(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileTagSetRequest>,
) -> Result<StatusCode, ApiError> {
    let tag_keys = normalize_tag_keys(request.tag_keys)?;
    state
        .indexers
        .search_profile_tag_allow(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.tag_public_ids.as_deref(),
            tag_keys.as_deref(),
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_tag_allow",
                SEARCH_PROFILE_TAG_ALLOW_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_tag_block(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileTagSetRequest>,
) -> Result<StatusCode, ApiError> {
    let tag_keys = normalize_tag_keys(request.tag_keys)?;
    state
        .indexers
        .search_profile_tag_block(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.tag_public_ids.as_deref(),
            tag_keys.as_deref(),
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_tag_block",
                SEARCH_PROFILE_TAG_BLOCK_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_search_profile_tag_prefer(
    Path(search_profile_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SearchProfileTagSetRequest>,
) -> Result<StatusCode, ApiError> {
    let tag_keys = normalize_tag_keys(request.tag_keys)?;
    state
        .indexers
        .search_profile_tag_prefer(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_profile_public_id,
            request.tag_public_ids.as_deref(),
            tag_keys.as_deref(),
        )
        .await
        .map_err(|err| {
            map_search_profile_error(
                "search_profile_tag_prefer",
                SEARCH_PROFILE_TAG_PREFER_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_search_profile_error(
    operation: &'static str,
    detail: &'static str,
    err: &SearchProfileServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        SearchProfileServiceErrorKind::Invalid => ApiError::bad_request(detail),
        SearchProfileServiceErrorKind::NotFound => ApiError::not_found(detail),
        SearchProfileServiceErrorKind::Conflict => ApiError::conflict(detail),
        SearchProfileServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        SearchProfileServiceErrorKind::Storage => ApiError::internal(detail),
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

fn normalize_tag_keys(tag_keys: Option<Vec<String>>) -> Result<Option<Vec<String>>, ApiError> {
    let Some(keys) = tag_keys else {
        return Ok(None);
    };

    if keys.len() > SEARCH_PROFILE_TAG_KEYS_MAX_LEN {
        let mut error = ApiError::bad_request(SEARCH_PROFILE_TAG_KEYS_TOO_LARGE);
        error = error.with_context_field("max_len", SEARCH_PROFILE_TAG_KEYS_MAX_LEN.to_string());
        return Err(error);
    }

    let mut normalized_len = 0_usize;
    let mut requested_bytes = 0_usize;
    for key in &keys {
        let trimmed = key.trim();
        if trimmed.len() > SEARCH_PROFILE_TAG_KEY_MAX_BYTES {
            let mut error = ApiError::bad_request(SEARCH_PROFILE_TAG_KEY_TOO_LARGE);
            error =
                error.with_context_field("max_len", SEARCH_PROFILE_TAG_KEY_MAX_BYTES.to_string());
            return Err(error);
        }
        if !trimmed.is_empty() {
            normalized_len = normalized_len.saturating_add(1);
            requested_bytes = requested_bytes.saturating_add(trimmed.len());
        }
    }
    let vec_overhead = mem::size_of::<String>().saturating_mul(normalized_len);
    requested_bytes = requested_bytes.saturating_add(vec_overhead);
    let capacity = normalized_len.min(SEARCH_PROFILE_TAG_KEYS_MAX_LEN);
    ensure_allocation_safe(requested_bytes)?;
    let mut normalized = checked_vec_capacity::<String>(capacity)?;
    for key in keys {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            normalized.push(trimmed.to_string());
        }
    }

    Ok(Some(normalized))
}

fn calculate_domain_key_bytes(values: &[String]) -> Result<usize, ApiError> {
    if values.len() > SEARCH_PROFILE_DOMAIN_KEYS_MAX_LEN {
        let mut error = ApiError::bad_request(SEARCH_PROFILE_DOMAIN_KEYS_TOO_LARGE);
        error = error.with_context_field("max_len", SEARCH_PROFILE_DOMAIN_KEYS_MAX_LEN.to_string());
        return Err(error);
    }
    let mut requested = mem::size_of::<String>().saturating_mul(values.len());
    for value in values {
        let trimmed = value.trim();
        if trimmed.len() > SEARCH_PROFILE_DOMAIN_KEY_MAX_BYTES {
            let mut error = ApiError::bad_request(SEARCH_PROFILE_DOMAIN_KEY_TOO_LARGE);
            error = error
                .with_context_field("max_len", SEARCH_PROFILE_DOMAIN_KEY_MAX_BYTES.to_string());
            return Err(error);
        }
        if !trimmed.is_empty() {
            requested = requested.saturating_add(trimmed.len());
        }
    }
    Ok(requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::{
        CategoryMappingServiceError, CategoryMappingServiceErrorKind, IndexerCfStateResetParams,
        IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind, IndexerFacade,
        IndexerInstanceFieldError, IndexerInstanceFieldErrorKind, IndexerInstanceFieldValueParams,
        IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
        IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams,
        RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind, RoutingPolicyServiceError,
        RoutingPolicyServiceErrorKind, SearchProfileServiceError, SearchProfileServiceErrorKind,
        SecretServiceError, SecretServiceErrorKind, TagServiceError, TagServiceErrorKind,
        TorznabInstanceCredentials, TorznabInstanceServiceError, TorznabInstanceServiceErrorKind,
    };
    use crate::config::ConfigFacade;
    use crate::models::{
        IndexerCfStateResponse, IndexerDefinitionResponse, IndexerInstanceTestFinalizeResponse,
        IndexerInstanceTestPrepareResponse, ProblemDetails,
    };
    use async_trait::async_trait;
    use axum::response::IntoResponse;
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        SettingsChangeset, SetupToken, TelemetryConfig, validate::default_local_networks,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig;

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "test".into(),
                mode: AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 8080,
                bind_addr: "127.0.0.1"
                    .parse()
                    .map_err(|_| ConfigError::InvalidBindAddr {
                        value: "127.0.0.1".to_string(),
                    })?,
                local_networks: default_local_networks(),
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            })
        }

        async fn issue_setup_token(&self, _: Duration, _: &str) -> ConfigResult<SetupToken> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn validate_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn consume_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn apply_changeset(
            &self,
            _: &str,
            _: &str,
            _: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "changeset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "snapshot".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn authenticate_api_key(&self, _: &str, _: &str) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(true)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct RecordingIndexers {
        last_display_name: Arc<Mutex<Option<String>>>,
    }

    fn api_state(indexers: Arc<dyn IndexerFacade>) -> Result<Arc<ApiState>, ApiError> {
        let telemetry = Metrics::new().map_err(|_| ApiError::internal("metrics init failed"))?;
        Ok(Arc::new(ApiState::new(
            Arc::new(StubConfig),
            indexers,
            telemetry,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        )))
    }

    async fn parse_problem(response: axum::response::Response) -> ProblemDetails {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        serde_json::from_slice(&body).unwrap_or_else(|_| ProblemDetails {
            kind: "invalid".to_string(),
            title: "invalid".to_string(),
            status: 0,
            detail: None,
            invalid_params: None,
            context: None,
        })
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
            display_name: &str,
            _is_default: Option<bool>,
            _page_size: Option<i32>,
            _default_media_domain_key: Option<&str>,
            _user_public_id: Option<Uuid>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            *self.last_display_name.lock().expect("lock") = Some(display_name.to_string());
            Ok(Uuid::new_v4())
        }

        async fn search_profile_update(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _display_name: Option<&str>,
            _page_size: Option<i32>,
        ) -> Result<Uuid, SearchProfileServiceError> {
            Ok(Uuid::new_v4())
        }

        async fn search_profile_set_default(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _page_size: Option<i32>,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_set_default_domain(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _default_media_domain_key: Option<&str>,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_set_domain_allowlist(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_add_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_remove_policy_set(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_indexer_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_indexer_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _indexer_instance_public_ids: &[Uuid],
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_tag_allow(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_tag_block(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
        }

        async fn search_profile_tag_prefer(
            &self,
            _actor_user_public_id: Uuid,
            _search_profile_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), SearchProfileServiceError> {
            Ok(())
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
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_update(
            &self,
            _params: IndexerInstanceUpdateParams<'_>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_media_domains(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_tags(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_set_value(
            &self,
            _params: IndexerInstanceFieldValueParams<'_>,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _field_name: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_reset(
            &self,
            _params: IndexerCfStateResetParams<'_>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_get(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_prepare(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_finalize(
            &self,
            _params: IndexerInstanceTestFinalizeParams<'_>,
        ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
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
    async fn create_search_profile_trims_name() {
        let indexers = RecordingIndexers::default();
        let state = api_state(Arc::new(indexers.clone())).expect("state");
        let request = SearchProfileCreateRequest {
            display_name: "  Profile  ".to_string(),
            is_default: None,
            page_size: None,
            default_media_domain_key: None,
            user_public_id: None,
        };

        let response = create_search_profile(State(state), Json(request)).await;
        assert!(response.is_ok());
        assert_eq!(
            indexers.last_display_name.lock().expect("lock").as_deref(),
            Some("Profile")
        );
    }

    #[tokio::test]
    async fn create_search_profile_conflict_maps_problem() {
        let state = api_state(Arc::new(ErrorIndexers)).expect("state");

        let request = SearchProfileCreateRequest {
            display_name: "Profile".to_string(),
            is_default: None,
            page_size: None,
            default_media_domain_key: None,
            user_public_id: None,
        };

        let response = create_search_profile(State(state), Json(request))
            .await
            .expect_err("expected conflict");
        let problem = parse_problem(response.into_response()).await;
        assert_eq!(
            problem.detail,
            Some(SEARCH_PROFILE_CREATE_FAILED.to_string())
        );
        assert_eq!(problem.status, 409);
        assert_eq!(
            problem.context.as_ref().and_then(|context| {
                context
                    .iter()
                    .find(|field| field.name == "error_code")
                    .map(|field| field.value.as_str())
            }),
            Some("search_profile_deleted")
        );
    }

    struct ErrorIndexers;

    #[async_trait]
    impl IndexerFacade for ErrorIndexers {
        async fn indexer_definition_list(
            &self,
            _actor_user_public_id: Uuid,
        ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
            Err(IndexerDefinitionServiceError::new(
                IndexerDefinitionServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_prepare(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_test_finalize(
            &self,
            _params: IndexerInstanceTestFinalizeParams<'_>,
        ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
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
            Err(
                SearchProfileServiceError::new(SearchProfileServiceErrorKind::Conflict)
                    .with_code("search_profile_deleted"),
            )
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
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_update(
            &self,
            _params: IndexerInstanceUpdateParams<'_>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_media_domains(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _media_domain_keys: &[String],
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_tags(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _tag_public_ids: Option<&[Uuid]>,
            _tag_keys: Option<&[String]>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_set_value(
            &self,
            _params: IndexerInstanceFieldValueParams<'_>,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
            _field_name: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_reset(
            &self,
            _params: IndexerCfStateResetParams<'_>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_get(
            &self,
            _actor_user_public_id: Uuid,
            _indexer_instance_public_id: Uuid,
        ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
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
}
