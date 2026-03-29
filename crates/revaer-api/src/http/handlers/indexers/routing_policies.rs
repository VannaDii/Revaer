//! Routing policy management endpoints for indexers.
//!
//! # Design
//! - Delegate routing policy operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use uuid::Uuid;

use crate::app::indexers::{RoutingPolicyServiceError, RoutingPolicyServiceErrorKind};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{
    RoutingPolicyCreateRequest, RoutingPolicyDetailResponse, RoutingPolicyParamSetRequest,
    RoutingPolicyResponse, RoutingPolicySecretBindRequest,
};

const ROUTING_POLICY_CREATE_FAILED: &str = "failed to create routing policy";
const ROUTING_POLICY_GET_FAILED: &str = "failed to fetch routing policy";
const ROUTING_POLICY_PARAM_SET_FAILED: &str = "failed to set routing policy parameter";
const ROUTING_POLICY_BIND_SECRET_FAILED: &str = "failed to bind routing policy secret";

pub(crate) async fn create_routing_policy(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<RoutingPolicyCreateRequest>,
) -> Result<(StatusCode, Json<RoutingPolicyResponse>), ApiError> {
    let display_name = request.display_name.trim();
    let mode = request.mode.trim();
    let routing_policy_public_id = state
        .indexers
        .routing_policy_create(SYSTEM_ACTOR_PUBLIC_ID, display_name, mode)
        .await
        .map_err(|err| {
            map_routing_policy_error("routing_policy_create", ROUTING_POLICY_CREATE_FAILED, &err)
        })?;

    Ok((
        StatusCode::CREATED,
        Json(RoutingPolicyResponse {
            routing_policy_public_id,
            display_name: display_name.to_string(),
            mode: mode.to_string(),
        }),
    ))
}

pub(crate) async fn get_routing_policy(
    State(state): State<Arc<ApiState>>,
    Path(routing_policy_public_id): Path<Uuid>,
) -> Result<Json<RoutingPolicyDetailResponse>, ApiError> {
    let response = state
        .indexers
        .routing_policy_get(SYSTEM_ACTOR_PUBLIC_ID, routing_policy_public_id)
        .await
        .map_err(|err| {
            map_routing_policy_error("routing_policy_get", ROUTING_POLICY_GET_FAILED, &err)
        })?;

    Ok(Json(response))
}

pub(crate) async fn set_routing_policy_param(
    State(state): State<Arc<ApiState>>,
    Path(routing_policy_public_id): Path<Uuid>,
    Json(request): Json<RoutingPolicyParamSetRequest>,
) -> Result<StatusCode, ApiError> {
    let param_key = request.param_key.trim();
    let value_plain = request.value_plain.as_deref().map(str::trim);
    state
        .indexers
        .routing_policy_set_param(
            SYSTEM_ACTOR_PUBLIC_ID,
            routing_policy_public_id,
            param_key,
            value_plain,
            request.value_int,
            request.value_bool,
        )
        .await
        .map_err(|err| {
            map_routing_policy_error(
                "routing_policy_set_param",
                ROUTING_POLICY_PARAM_SET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn bind_routing_policy_secret(
    State(state): State<Arc<ApiState>>,
    Path(routing_policy_public_id): Path<Uuid>,
    Json(request): Json<RoutingPolicySecretBindRequest>,
) -> Result<StatusCode, ApiError> {
    let param_key = request.param_key.trim();
    state
        .indexers
        .routing_policy_bind_secret(
            SYSTEM_ACTOR_PUBLIC_ID,
            routing_policy_public_id,
            param_key,
            request.secret_public_id,
        )
        .await
        .map_err(|err| {
            map_routing_policy_error(
                "routing_policy_bind_secret",
                ROUTING_POLICY_BIND_SECRET_FAILED,
                &err,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_routing_policy_error(
    operation: &'static str,
    detail: &'static str,
    err: &RoutingPolicyServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        RoutingPolicyServiceErrorKind::Invalid => ApiError::bad_request(detail),
        RoutingPolicyServiceErrorKind::NotFound => ApiError::not_found(detail),
        RoutingPolicyServiceErrorKind::Conflict => ApiError::conflict(detail),
        RoutingPolicyServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        RoutingPolicyServiceErrorKind::Storage => ApiError::internal(detail),
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
        IndexerInstanceTestPrepareResponse, ProblemDetails, RoutingPolicyDetailResponse,
        RoutingPolicyParameterResponse,
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Create {
            actor: Uuid,
            display_name: String,
            mode: String,
        },
        SetParam {
            actor: Uuid,
            routing_policy_public_id: Uuid,
            param_key: String,
            value_plain: Option<String>,
            value_int: Option<i32>,
            value_bool: Option<bool>,
        },
        BindSecret {
            actor: Uuid,
            routing_policy_public_id: Uuid,
            param_key: String,
            secret_public_id: Uuid,
        },
        Get {
            actor: Uuid,
            routing_policy_public_id: Uuid,
        },
    }

    struct RecordingIndexers {
        calls: Mutex<Vec<Call>>,
        create_result: Mutex<Option<Result<Uuid, RoutingPolicyServiceError>>>,
        get_result: Mutex<Option<Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError>>>,
    }

    impl RecordingIndexers {
        fn with_create_result(result: Result<Uuid, RoutingPolicyServiceError>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                create_result: Mutex::new(Some(result)),
                get_result: Mutex::new(None),
            }
        }

        fn with_get_result(
            result: Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError>,
        ) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                create_result: Mutex::new(None),
                get_result: Mutex::new(Some(result)),
            }
        }

        fn take_calls(&self) -> Vec<Call> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .drain(..)
                .collect()
        }

        fn take_create_result(&self) -> Result<Uuid, RoutingPolicyServiceError> {
            self.create_result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Err(RoutingPolicyServiceError::new(
                        RoutingPolicyServiceErrorKind::Storage,
                    ))
                })
        }

        fn take_get_result(
            &self,
        ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
            self.get_result
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| {
                    Err(RoutingPolicyServiceError::new(
                        RoutingPolicyServiceErrorKind::Storage,
                    ))
                })
        }
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
            actor_user_public_id: Uuid,
            display_name: &str,
            mode: &str,
        ) -> Result<Uuid, RoutingPolicyServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(Call::Create {
                    actor: actor_user_public_id,
                    display_name: display_name.to_string(),
                    mode: mode.to_string(),
                });
            self.take_create_result()
        }

        async fn routing_policy_set_param(
            &self,
            actor_user_public_id: Uuid,
            routing_policy_public_id: Uuid,
            param_key: &str,
            value_plain: Option<&str>,
            value_int: Option<i32>,
            value_bool: Option<bool>,
        ) -> Result<(), RoutingPolicyServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(Call::SetParam {
                    actor: actor_user_public_id,
                    routing_policy_public_id,
                    param_key: param_key.to_string(),
                    value_plain: value_plain.map(str::to_string),
                    value_int,
                    value_bool,
                });
            Ok(())
        }

        async fn routing_policy_bind_secret(
            &self,
            actor_user_public_id: Uuid,
            routing_policy_public_id: Uuid,
            param_key: &str,
            secret_public_id: Uuid,
        ) -> Result<(), RoutingPolicyServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(Call::BindSecret {
                    actor: actor_user_public_id,
                    routing_policy_public_id,
                    param_key: param_key.to_string(),
                    secret_public_id,
                });
            Ok(())
        }

        async fn routing_policy_get(
            &self,
            actor_user_public_id: Uuid,
            routing_policy_public_id: Uuid,
        ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(Call::Get {
                    actor: actor_user_public_id,
                    routing_policy_public_id,
                });
            self.take_get_result()
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
    }

    struct ErrorIndexers {
        routing_error: RoutingPolicyServiceError,
        category_mapping_error: CategoryMappingServiceError,
    }

    #[async_trait]
    impl IndexerFacade for ErrorIndexers {
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
            Err(self.routing_error.clone())
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
            Err(self.routing_error.clone())
        }

        async fn routing_policy_bind_secret(
            &self,
            _actor_user_public_id: Uuid,
            _routing_policy_public_id: Uuid,
            _param_key: &str,
            _secret_public_id: Uuid,
        ) -> Result<(), RoutingPolicyServiceError> {
            Err(self.routing_error.clone())
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

        async fn tracker_category_mapping_upsert(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(self.category_mapping_error.clone())
        }

        async fn tracker_category_mapping_delete(
            &self,
            _params: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(self.category_mapping_error.clone())
        }

        async fn media_domain_mapping_upsert(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
            _is_primary: Option<bool>,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(self.category_mapping_error.clone())
        }

        async fn media_domain_mapping_delete(
            &self,
            _actor_user_public_id: Uuid,
            _media_domain_key: &str,
            _torznab_cat_id: i32,
        ) -> Result<(), CategoryMappingServiceError> {
            Err(self.category_mapping_error.clone())
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

    #[tokio::test]
    async fn create_routing_policy_trims_values_and_returns_payload() -> Result<(), ApiError> {
        let routing_policy_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::with_create_result(Ok(
            routing_policy_public_id,
        )));
        let state = api_state(indexers.clone())?;

        let request = RoutingPolicyCreateRequest {
            display_name: " Proxy ".to_string(),
            mode: " direct ".to_string(),
        };

        let (status, Json(response)) = create_routing_policy(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.routing_policy_public_id, routing_policy_public_id);
        assert_eq!(response.display_name, "Proxy");
        assert_eq!(response.mode, "direct");

        let calls = indexers.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            Call::Create {
                actor: SYSTEM_ACTOR_PUBLIC_ID,
                display_name: "Proxy".to_string(),
                mode: "direct".to_string(),
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_routing_policy_returns_payload() -> Result<(), ApiError> {
        let routing_policy_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::with_get_result(Ok(
            RoutingPolicyDetailResponse {
                routing_policy_public_id,
                display_name: "Proxy".to_string(),
                mode: "http_proxy".to_string(),
                rate_limit_policy_public_id: Some(Uuid::new_v4()),
                rate_limit_display_name: Some("Proxy Budget".to_string()),
                rate_limit_requests_per_minute: Some(90),
                rate_limit_burst: Some(15),
                rate_limit_concurrent_requests: Some(3),
                parameters: vec![RoutingPolicyParameterResponse {
                    param_key: "proxy_host".to_string(),
                    value_plain: Some("proxy.internal".to_string()),
                    value_int: None,
                    value_bool: None,
                    secret_public_id: None,
                    secret_binding_name: None,
                }],
            },
        )));
        let state = api_state(indexers.clone())?;

        let Json(response) =
            get_routing_policy(State(state), Path(routing_policy_public_id)).await?;
        assert_eq!(response.routing_policy_public_id, routing_policy_public_id);
        assert_eq!(response.parameters.len(), 1);

        let calls = indexers.take_calls();
        assert_eq!(
            calls,
            vec![Call::Get {
                actor: SYSTEM_ACTOR_PUBLIC_ID,
                routing_policy_public_id,
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_routing_policy_param_trims_values_and_returns_no_content() -> Result<(), ApiError>
    {
        let indexers = Arc::new(RecordingIndexers::with_create_result(Ok(Uuid::new_v4())));
        let state = api_state(indexers.clone())?;
        let routing_policy_public_id = Uuid::new_v4();

        let request = RoutingPolicyParamSetRequest {
            param_key: " proxy_host ".to_string(),
            value_plain: Some(" localhost ".to_string()),
            value_int: None,
            value_bool: None,
        };

        let status =
            set_routing_policy_param(State(state), Path(routing_policy_public_id), Json(request))
                .await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            Call::SetParam {
                actor: SYSTEM_ACTOR_PUBLIC_ID,
                routing_policy_public_id,
                param_key: "proxy_host".to_string(),
                value_plain: Some("localhost".to_string()),
                value_int: None,
                value_bool: None,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn bind_routing_policy_secret_returns_no_content() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::with_create_result(Ok(Uuid::new_v4())));
        let state = api_state(indexers.clone())?;
        let routing_policy_public_id = Uuid::new_v4();
        let secret_public_id = Uuid::new_v4();

        let request = RoutingPolicySecretBindRequest {
            param_key: " http_proxy_auth ".to_string(),
            secret_public_id,
        };

        let status =
            bind_routing_policy_secret(State(state), Path(routing_policy_public_id), Json(request))
                .await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers.take_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            Call::BindSecret {
                actor: SYSTEM_ACTOR_PUBLIC_ID,
                routing_policy_public_id,
                param_key: "http_proxy_auth".to_string(),
                secret_public_id,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_routing_policy_conflict_maps_conflict() -> Result<(), ApiError> {
        let indexers = Arc::new(ErrorIndexers {
            routing_error: RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::Conflict)
                .with_code("display_name_already_exists"),
            category_mapping_error: CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ),
        });
        let state = api_state(indexers)?;

        let request = RoutingPolicyCreateRequest {
            display_name: "Routing".to_string(),
            mode: "direct".to_string(),
        };

        let err = create_routing_policy(State(state), Json(request))
            .await
            .err()
            .unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }

    #[tokio::test]
    async fn bind_routing_policy_secret_not_found_maps_problem_context() -> Result<(), ApiError> {
        let indexers = Arc::new(ErrorIndexers {
            routing_error: RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
                .with_code("secret_not_found"),
            category_mapping_error: CategoryMappingServiceError::new(
                CategoryMappingServiceErrorKind::Storage,
            ),
        });
        let state = api_state(indexers)?;

        let err = bind_routing_policy_secret(
            State(state),
            Path(Uuid::new_v4()),
            Json(RoutingPolicySecretBindRequest {
                param_key: "http_proxy_auth".to_string(),
                secret_public_id: Uuid::new_v4(),
            }),
        )
        .await
        .err()
        .unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(
            problem.context.as_ref().and_then(|context| {
                context
                    .iter()
                    .find(|field| field.name == "error_code")
                    .map(|field| field.value.as_str())
            }),
            Some("secret_not_found")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_routing_policy_not_found_maps_problem_context() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::with_get_result(Err(
            RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
                .with_code("routing_policy_not_found"),
        )));
        let state = api_state(indexers)?;

        let err = get_routing_policy(State(state), Path(Uuid::new_v4()))
            .await
            .err()
            .unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(ROUTING_POLICY_GET_FAILED));
        Ok(())
    }
}
