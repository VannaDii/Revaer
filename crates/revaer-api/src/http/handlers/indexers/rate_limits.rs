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
use crate::http::handlers::indexers::normalization::trim_and_filter_empty;
use crate::models::{
    RateLimitPolicyAssignmentRequest, RateLimitPolicyCreateRequest, RateLimitPolicyResponse,
    RateLimitPolicyUpdateRequest,
};

const RATE_LIMIT_CREATE_FAILED: &str = "failed to create rate limit policy";
const RATE_LIMIT_UPDATE_FAILED: &str = "failed to update rate limit policy";
const RATE_LIMIT_DELETE_FAILED: &str = "failed to delete rate limit policy";
const RATE_LIMIT_ASSIGN_FAILED: &str = "failed to assign rate limit policy";

pub(crate) async fn create_rate_limit_policy(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RateLimitPolicyCreateRequest>,
) -> Result<(StatusCode, Json<RateLimitPolicyResponse>), ApiError> {
    let display_name = request.display_name.trim();
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
    use crate::app::state::ApiState;
    use crate::config::ConfigFacade;
    use crate::models::{
        IndexerCfStateResponse, IndexerDefinitionResponse, IndexerInstanceTestFinalizeResponse,
        IndexerInstanceTestPrepareResponse, ProblemDetails,
    };
    use async_trait::async_trait;
    use axum::{Router, body::Body, http::Request};
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        SettingsChangeset, SetupToken, TelemetryConfig, validate::default_local_networks,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tower::ServiceExt;
    use uuid::Uuid;

    type RateLimitUpdateCall = (Uuid, Option<String>, Option<i32>, Option<i32>, Option<i32>);

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
            Ok(false)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "factory_reset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }
    }

    #[derive(Default)]
    struct RecordingIndexers {
        created: Mutex<Vec<(String, i32, i32, i32)>>,
        updates: Mutex<Vec<RateLimitUpdateCall>>,
        instance_assignments: Mutex<Vec<(Uuid, Option<Uuid>)>>,
    }

    #[async_trait]
    impl IndexerFacade for RecordingIndexers {
        async fn indexer_definition_list(
            &self,
            _actor_user_public_id: Uuid,
        ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
            Err(IndexerDefinitionServiceError::new(
                IndexerDefinitionServiceErrorKind::Storage,
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
            display_name: &str,
            rpm: i32,
            burst: i32,
            concurrent: i32,
        ) -> Result<Uuid, RateLimitPolicyServiceError> {
            self.created
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((display_name.to_string(), rpm, burst, concurrent));
            Ok(Uuid::new_v4())
        }

        async fn rate_limit_policy_update(
            &self,
            _actor_user_public_id: Uuid,
            rate_limit_policy_public_id: Uuid,
            display_name: Option<&str>,
            rpm: Option<i32>,
            burst: Option<i32>,
            concurrent: Option<i32>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            self.updates
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((
                    rate_limit_policy_public_id,
                    display_name.map(str::to_string),
                    rpm,
                    burst,
                    concurrent,
                ));
            Ok(())
        }

        async fn rate_limit_policy_soft_delete(
            &self,
            _actor_user_public_id: Uuid,
            _rate_limit_policy_public_id: Uuid,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Ok(())
        }

        async fn indexer_instance_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            indexer_instance_public_id: Uuid,
            rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            self.instance_assignments
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((indexer_instance_public_id, rate_limit_policy_public_id));
            Ok(())
        }

        async fn routing_policy_set_rate_limit_policy(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _rate_limit_policy_public_id: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Ok(())
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

    struct ErrorIndexers;

    #[async_trait]
    impl IndexerFacade for ErrorIndexers {
        async fn indexer_definition_list(
            &self,
            _: Uuid,
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

        async fn tag_create(&self, _: Uuid, _: &str, _: &str) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_update(
            &self,
            _: Uuid,
            _: Option<Uuid>,
            _: Option<&str>,
            _: &str,
        ) -> Result<Uuid, TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn tag_delete(
            &self,
            _: Uuid,
            _: Option<Uuid>,
            _: Option<&str>,
        ) -> Result<(), TagServiceError> {
            Err(TagServiceError::new(TagServiceErrorKind::Storage))
        }

        async fn routing_policy_create(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
        ) -> Result<Uuid, RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_set_param(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
            _: Option<&str>,
            _: Option<i32>,
            _: Option<bool>,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn routing_policy_bind_secret(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
            _: Uuid,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(RoutingPolicyServiceError::new(
                RoutingPolicyServiceErrorKind::Storage,
            ))
        }

        async fn rate_limit_policy_create(
            &self,
            _: Uuid,
            _: &str,
            _: i32,
            _: i32,
            _: i32,
        ) -> Result<Uuid, RateLimitPolicyServiceError> {
            Err(
                RateLimitPolicyServiceError::new(RateLimitPolicyServiceErrorKind::Invalid)
                    .with_code("display_name_missing"),
            )
        }

        async fn rate_limit_policy_update(
            &self,
            _: Uuid,
            _: Uuid,
            _: Option<&str>,
            _: Option<i32>,
            _: Option<i32>,
            _: Option<i32>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Invalid,
            ))
        }

        async fn rate_limit_policy_soft_delete(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::NotFound,
            ))
        }

        async fn indexer_instance_set_rate_limit_policy(
            &self,
            _: Uuid,
            _: Uuid,
            _: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Conflict,
            ))
        }

        async fn routing_policy_set_rate_limit_policy(
            &self,
            _: Uuid,
            _: Uuid,
            _: Option<Uuid>,
        ) -> Result<(), RateLimitPolicyServiceError> {
            Err(RateLimitPolicyServiceError::new(
                RateLimitPolicyServiceErrorKind::Unauthorized,
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
            _: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn tracker_category_mapping_delete(
            &self,
            _: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_upsert(
            &self,
            _: Uuid,
            _: &str,
            _: i32,
            _: Option<bool>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn media_domain_mapping_delete(
            &self,
            _: Uuid,
            _: &str,
            _: i32,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_create(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_rotate_key(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_enable_disable(
            &self,
            _: Uuid,
            _: Uuid,
            _: bool,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn torznab_instance_soft_delete(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<(), TorznabInstanceServiceError> {
            Err(TorznabInstanceServiceError::new(
                TorznabInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_create(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
            _: Option<i32>,
            _: Option<&str>,
            _: Option<Uuid>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_update(
            &self,
            _: IndexerInstanceUpdateParams<'_>,
        ) -> Result<Uuid, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_media_domains(
            &self,
            _: Uuid,
            _: Uuid,
            _: &[String],
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_set_tags(
            &self,
            _: Uuid,
            _: Uuid,
            _: Option<&[Uuid]>,
            _: Option<&[String]>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_set_value(
            &self,
            _: IndexerInstanceFieldValueParams<'_>,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_instance_field_bind_secret(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
            _: Uuid,
        ) -> Result<(), IndexerInstanceFieldError> {
            Err(IndexerInstanceFieldError::new(
                IndexerInstanceFieldErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_reset(
            &self,
            _: IndexerCfStateResetParams<'_>,
        ) -> Result<(), IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn indexer_cf_state_get(
            &self,
            _: Uuid,
            _: Uuid,
        ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
            Err(IndexerInstanceServiceError::new(
                IndexerInstanceServiceErrorKind::Storage,
            ))
        }

        async fn secret_create(
            &self,
            _: Uuid,
            _: &str,
            _: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_rotate(
            &self,
            _: Uuid,
            _: Uuid,
            _: &str,
        ) -> Result<Uuid, SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }

        async fn secret_revoke(&self, _: Uuid, _: Uuid) -> Result<(), SecretServiceError> {
            Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
        }
    }

    fn build_state(indexers: Arc<dyn IndexerFacade>) -> Arc<ApiState> {
        let telemetry = Metrics::new().expect("metrics init failed");
        Arc::new(ApiState::new(
            Arc::new(StubConfig),
            indexers,
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::default(),
            None,
        ))
    }

    #[tokio::test]
    async fn create_rate_limit_policy_trims_name_and_returns_id() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = build_state(indexers.clone());
        let payload = json!({
            "display_name": "  Default  ",
            "rpm": 60,
            "burst": 10,
            "concurrent": 2
        });
        let response =
            create_rate_limit_policy(State(state), Json(serde_json::from_value(payload).unwrap()))
                .await
                .unwrap();

        assert_eq!(response.0, StatusCode::CREATED);
        let created = indexers
            .created
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].0, "Default");
        assert_eq!(created[0].1, 60);
    }

    #[tokio::test]
    async fn update_rate_limit_policy_records_updates() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = build_state(indexers.clone());
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
        .unwrap();

        assert_eq!(response, StatusCode::NO_CONTENT);
        let updates = indexers
            .updates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, policy_id);
        assert_eq!(updates[0].1.as_deref(), Some("Updated"));
    }

    #[tokio::test]
    async fn update_rate_limit_policy_filters_whitespace_only_display_name() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = build_state(indexers.clone());
        let payload = json!({
            "display_name": "   "
        });
        let policy_id = Uuid::new_v4();

        let response = update_rate_limit_policy(
            Path(policy_id),
            State(state),
            Json(serde_json::from_value(payload).unwrap()),
        )
        .await
        .unwrap();

        assert_eq!(response, StatusCode::NO_CONTENT);
        let updates = indexers
            .updates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, policy_id);
        assert_eq!(updates[0].1, None);
    }

    #[tokio::test]
    async fn assign_rate_limit_policy_to_indexer_instance() {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = build_state(indexers.clone());
        let policy_id = Uuid::new_v4();
        let instance_id = Uuid::new_v4();
        let payload = json!({ "rate_limit_policy_public_id": policy_id });
        let response = set_indexer_instance_rate_limit(
            Path(instance_id),
            State(state),
            Json(serde_json::from_value(payload).unwrap()),
        )
        .await
        .unwrap();

        assert_eq!(response, StatusCode::NO_CONTENT);
        let assignments = indexers
            .instance_assignments
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(assignments, vec![(instance_id, Some(policy_id))]);
    }

    #[tokio::test]
    async fn error_mapping_surfaces_problem_details() {
        let state = build_state(Arc::new(ErrorIndexers));
        let router = Router::new()
            .route(
                "/v1/indexers/rate-limits",
                axum::routing::post(create_rate_limit_policy),
            )
            .with_state(state);
        let request = Request::builder()
            .method("POST")
            .uri("/v1/indexers/rate-limits")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "display_name": "",
                    "rpm": 1,
                    "burst": 1,
                    "concurrent": 1
                })
                .to_string(),
            ))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        let details: ProblemDetails = serde_json::from_slice(&body).unwrap();
        assert_eq!(details.detail.as_deref(), Some(RATE_LIMIT_CREATE_FAILED));
    }
}
