//! Policy set and rule endpoints for indexers.
//!
//! # Design
//! - Delegate policy operations to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::{mem, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};

use crate::app::indexers::{
    PolicyRuleCreateParams, PolicyRuleValueItem, PolicyServiceError, PolicyServiceErrorKind,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::allocation::{checked_vec_capacity, ensure_allocation_safe};
use crate::models::{
    PolicyRuleCreateRequest, PolicyRuleReorderRequest, PolicyRuleResponse,
    PolicyRuleValueItemRequest, PolicySetCreateRequest, PolicySetReorderRequest, PolicySetResponse,
    PolicySetUpdateRequest,
};

const POLICY_SET_CREATE_FAILED: &str = "failed to create policy set";
const POLICY_SET_UPDATE_FAILED: &str = "failed to update policy set";
const POLICY_SET_ENABLE_FAILED: &str = "failed to enable policy set";
const POLICY_SET_DISABLE_FAILED: &str = "failed to disable policy set";
const POLICY_SET_REORDER_FAILED: &str = "failed to reorder policy sets";
const POLICY_RULE_CREATE_FAILED: &str = "failed to create policy rule";
const POLICY_RULE_ENABLE_FAILED: &str = "failed to enable policy rule";
const POLICY_RULE_DISABLE_FAILED: &str = "failed to disable policy rule";
const POLICY_RULE_REORDER_FAILED: &str = "failed to reorder policy rules";
const POLICY_RULE_EXPIRES_AT_INVALID: &str = "expires_at must be an RFC3339 timestamp";
const POLICY_RULE_VALUE_SET_TOO_LARGE: &str = "policy rule value set exceeds maximum size";
const POLICY_RULE_TEXT_TOO_LARGE: &str = "policy rule text exceeds maximum size";
const POLICY_RULE_VALUE_TEXT_TOO_LARGE: &str = "policy rule value text exceeds maximum size";
const POLICY_RULE_VALUE_SET_MAX_LEN: usize = 1024;
const POLICY_RULE_TEXT_MAX_BYTES: usize = 4096;
const POLICY_RULE_VALUE_TEXT_MAX_BYTES: usize = 4096;

pub(crate) async fn create_policy_set(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<PolicySetCreateRequest>,
) -> Result<(StatusCode, Json<PolicySetResponse>), ApiError> {
    let display_name = request.display_name.trim();
    let scope = request.scope.trim();
    let policy_set_public_id = state
        .indexers
        .policy_set_create(SYSTEM_ACTOR_PUBLIC_ID, display_name, scope, request.enabled)
        .await
        .map_err(|err| map_policy_error("policy_set_create", POLICY_SET_CREATE_FAILED, &err))?;

    Ok((
        StatusCode::CREATED,
        Json(PolicySetResponse {
            policy_set_public_id,
        }),
    ))
}

pub(crate) async fn update_policy_set(
    Path(policy_set_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<PolicySetUpdateRequest>,
) -> Result<Json<PolicySetResponse>, ApiError> {
    let display_name = request.display_name.as_deref().map(str::trim);
    let policy_set_public_id = state
        .indexers
        .policy_set_update(SYSTEM_ACTOR_PUBLIC_ID, policy_set_public_id, display_name)
        .await
        .map_err(|err| map_policy_error("policy_set_update", POLICY_SET_UPDATE_FAILED, &err))?;

    Ok(Json(PolicySetResponse {
        policy_set_public_id,
    }))
}

pub(crate) async fn enable_policy_set(
    Path(policy_set_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_set_enable(SYSTEM_ACTOR_PUBLIC_ID, policy_set_public_id)
        .await
        .map_err(|err| map_policy_error("policy_set_enable", POLICY_SET_ENABLE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn disable_policy_set(
    Path(policy_set_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_set_disable(SYSTEM_ACTOR_PUBLIC_ID, policy_set_public_id)
        .await
        .map_err(|err| map_policy_error("policy_set_disable", POLICY_SET_DISABLE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn reorder_policy_sets(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<PolicySetReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_set_reorder(
            SYSTEM_ACTOR_PUBLIC_ID,
            &request.ordered_policy_set_public_ids,
        )
        .await
        .map_err(|err| map_policy_error("policy_set_reorder", POLICY_SET_REORDER_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn create_policy_rule(
    Path(policy_set_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<PolicyRuleCreateRequest>,
) -> Result<(StatusCode, Json<PolicyRuleResponse>), ApiError> {
    ensure_policy_rule_string_allocation_safe(&request)?;
    let expires_at = request
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| DateTime::parse_from_rfc3339(value).map(|parsed| parsed.with_timezone(&Utc)))
        .transpose()
        .map_err(|_| ApiError::bad_request(POLICY_RULE_EXPIRES_AT_INVALID))?;
    let value_set_items = normalize_value_set_items(request.value_set_items)?;
    let match_value_text =
        trim_and_validate_optional_policy_text(request.match_value_text, "match_value_text")?;
    let rationale = trim_and_validate_optional_policy_text(request.rationale, "rationale")?;
    let params = PolicyRuleCreateParams {
        actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
        policy_set_public_id,
        rule_type: request.rule_type.trim().to_string(),
        match_field: request.match_field.trim().to_string(),
        match_operator: request.match_operator.trim().to_string(),
        sort_order: request.sort_order,
        match_value_text,
        match_value_int: request.match_value_int,
        match_value_uuid: request.match_value_uuid,
        value_set_items,
        action: request.action.trim().to_string(),
        severity: request.severity.trim().to_string(),
        is_case_insensitive: request.is_case_insensitive,
        rationale,
        expires_at,
    };
    let policy_rule_public_id = state
        .indexers
        .policy_rule_create(params)
        .await
        .map_err(|err| map_policy_error("policy_rule_create", POLICY_RULE_CREATE_FAILED, &err))?;

    Ok((
        StatusCode::CREATED,
        Json(PolicyRuleResponse {
            policy_rule_public_id,
        }),
    ))
}

pub(crate) async fn enable_policy_rule(
    Path(policy_rule_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_rule_enable(SYSTEM_ACTOR_PUBLIC_ID, policy_rule_public_id)
        .await
        .map_err(|err| map_policy_error("policy_rule_enable", POLICY_RULE_ENABLE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn disable_policy_rule(
    Path(policy_rule_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_rule_disable(SYSTEM_ACTOR_PUBLIC_ID, policy_rule_public_id)
        .await
        .map_err(|err| map_policy_error("policy_rule_disable", POLICY_RULE_DISABLE_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn reorder_policy_rules(
    Path(policy_set_public_id): Path<uuid::Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<PolicyRuleReorderRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .indexers
        .policy_rule_reorder(
            SYSTEM_ACTOR_PUBLIC_ID,
            policy_set_public_id,
            &request.ordered_policy_rule_public_ids,
        )
        .await
        .map_err(|err| map_policy_error("policy_rule_reorder", POLICY_RULE_REORDER_FAILED, &err))?;

    Ok(StatusCode::NO_CONTENT)
}

fn map_value_item(item: PolicyRuleValueItemRequest) -> Result<PolicyRuleValueItem, ApiError> {
    let value_text = trim_and_validate_optional_value_text(item.value_text)?;
    Ok(PolicyRuleValueItem {
        value_text,
        value_int: item.value_int,
        value_bigint: item.value_bigint,
        value_uuid: item.value_uuid,
    })
}

fn trim_and_validate_optional_value_text(
    value: Option<String>,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.len() > POLICY_RULE_VALUE_TEXT_MAX_BYTES {
        let mut error = ApiError::bad_request(POLICY_RULE_VALUE_TEXT_TOO_LARGE);
        error = error.with_context_field("field", "value_text");
        error = error.with_context_field("max_len", POLICY_RULE_VALUE_TEXT_MAX_BYTES.to_string());
        return Err(error);
    }
    let requested_bytes = mem::size_of::<String>().saturating_add(trimmed.len());
    ensure_allocation_safe(requested_bytes)?;
    Ok(Some(trimmed.to_string()))
}

fn ensure_policy_rule_string_allocation_safe(
    request: &PolicyRuleCreateRequest,
) -> Result<(), ApiError> {
    let mut requested_bytes = mem::size_of::<String>().saturating_mul(5);
    requested_bytes = requested_bytes.saturating_add(validate_policy_rule_text_len(
        request.rule_type.trim(),
        "rule_type",
    )?);
    requested_bytes = requested_bytes.saturating_add(validate_policy_rule_text_len(
        request.match_field.trim(),
        "match_field",
    )?);
    requested_bytes = requested_bytes.saturating_add(validate_policy_rule_text_len(
        request.match_operator.trim(),
        "match_operator",
    )?);
    requested_bytes = requested_bytes.saturating_add(validate_policy_rule_text_len(
        request.action.trim(),
        "action",
    )?);
    requested_bytes = requested_bytes.saturating_add(validate_policy_rule_text_len(
        request.severity.trim(),
        "severity",
    )?);

    if let Some(value) = request.match_value_text.as_deref() {
        let trimmed = value.trim();
        let trimmed_len = validate_policy_rule_text_len(trimmed, "match_value_text")?;
        requested_bytes = requested_bytes.saturating_add(mem::size_of::<String>());
        requested_bytes = requested_bytes.saturating_add(trimmed_len);
    }
    if let Some(value) = request.rationale.as_deref() {
        let trimmed = value.trim();
        let trimmed_len = validate_policy_rule_text_len(trimmed, "rationale")?;
        requested_bytes = requested_bytes.saturating_add(mem::size_of::<String>());
        requested_bytes = requested_bytes.saturating_add(trimmed_len);
    }

    ensure_allocation_safe(requested_bytes)
}

fn validate_policy_rule_text_len(value: &str, field: &'static str) -> Result<usize, ApiError> {
    if value.len() > POLICY_RULE_TEXT_MAX_BYTES {
        let mut error = ApiError::bad_request(POLICY_RULE_TEXT_TOO_LARGE);
        error = error.with_context_field("field", field);
        error = error.with_context_field("max_len", POLICY_RULE_TEXT_MAX_BYTES.to_string());
        return Err(error);
    }
    Ok(value.len())
}

fn trim_and_validate_optional_policy_text(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    let trimmed_len = validate_policy_rule_text_len(trimmed, field)?;
    let requested_bytes = mem::size_of::<String>().saturating_add(trimmed_len);
    ensure_allocation_safe(requested_bytes)?;
    Ok(Some(trimmed.to_string()))
}

fn normalize_value_set_items(
    value_set_items: Option<Vec<PolicyRuleValueItemRequest>>,
) -> Result<Option<Vec<PolicyRuleValueItem>>, ApiError> {
    let Some(items) = value_set_items else {
        return Ok(None);
    };

    if items.len() > POLICY_RULE_VALUE_SET_MAX_LEN {
        let mut error = ApiError::bad_request(POLICY_RULE_VALUE_SET_TOO_LARGE);
        error = error.with_context_field("max_len", POLICY_RULE_VALUE_SET_MAX_LEN.to_string());
        return Err(error);
    }

    let requested_bytes = calculate_value_item_bytes(&items)?;
    let capacity = items.len().min(POLICY_RULE_VALUE_SET_MAX_LEN);
    ensure_allocation_safe(requested_bytes)?;
    let mut normalized = checked_vec_capacity::<PolicyRuleValueItem>(capacity)?;
    for item in items {
        normalized.push(map_value_item(item)?);
    }
    Ok(Some(normalized))
}

fn calculate_value_item_bytes(items: &[PolicyRuleValueItemRequest]) -> Result<usize, ApiError> {
    let mut requested = mem::size_of::<PolicyRuleValueItem>().saturating_mul(items.len());
    for item in items {
        if let Some(text) = item.value_text.as_deref() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                if trimmed.len() > POLICY_RULE_VALUE_TEXT_MAX_BYTES {
                    let mut error = ApiError::bad_request(POLICY_RULE_VALUE_TEXT_TOO_LARGE);
                    error = error.with_context_field("field", "value_text");
                    error = error.with_context_field(
                        "max_len",
                        POLICY_RULE_VALUE_TEXT_MAX_BYTES.to_string(),
                    );
                    return Err(error);
                }
                requested = requested.saturating_add(trimmed.len());
            }
        }
    }
    Ok(requested)
}

fn map_policy_error(
    operation: &'static str,
    detail: &'static str,
    err: &PolicyServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        PolicyServiceErrorKind::Invalid => ApiError::bad_request(detail),
        PolicyServiceErrorKind::NotFound => ApiError::not_found(detail),
        PolicyServiceErrorKind::Conflict => ApiError::conflict(detail),
        PolicyServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        PolicyServiceErrorKind::Storage => ApiError::internal(detail),
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

    type PolicySetCreateCall = (Uuid, String, String, Option<bool>);

    #[derive(Default)]
    struct RecordingIndexers {
        policy_calls: Mutex<Vec<PolicySetCreateCall>>,
        policy_rule_calls: Mutex<Vec<PolicyRuleCreateParams>>,
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

        async fn policy_set_create(
            &self,
            actor_user_public_id: Uuid,
            display_name: &str,
            scope: &str,
            enabled: Option<bool>,
        ) -> Result<Uuid, PolicyServiceError> {
            self.policy_calls.lock().expect("lock poisoned").push((
                actor_user_public_id,
                display_name.to_string(),
                scope.to_string(),
                enabled,
            ));
            Ok(Uuid::new_v4())
        }

        async fn policy_rule_create(
            &self,
            params: PolicyRuleCreateParams,
        ) -> Result<Uuid, PolicyServiceError> {
            self.policy_rule_calls
                .lock()
                .expect("lock poisoned")
                .push(params);
            Ok(Uuid::new_v4())
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

    struct ErrorIndexers {
        error: PolicyServiceError,
    }

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

        async fn policy_set_create(
            &self,
            _actor_user_public_id: Uuid,
            _display_name: &str,
            _scope: &str,
            _enabled: Option<bool>,
        ) -> Result<Uuid, PolicyServiceError> {
            Err(self.error.clone())
        }

        async fn policy_set_enable(
            &self,
            _actor_user_public_id: Uuid,
            _policy_set_public_id: Uuid,
        ) -> Result<(), PolicyServiceError> {
            Err(self.error.clone())
        }

        async fn policy_rule_enable(
            &self,
            _actor_user_public_id: Uuid,
            _policy_rule_public_id: Uuid,
        ) -> Result<(), PolicyServiceError> {
            Err(self.error.clone())
        }

        async fn policy_rule_disable(
            &self,
            _actor_user_public_id: Uuid,
            _policy_rule_public_id: Uuid,
        ) -> Result<(), PolicyServiceError> {
            Err(self.error.clone())
        }

        async fn policy_rule_reorder(
            &self,
            _actor_user_public_id: Uuid,
            _policy_set_public_id: Uuid,
            _ordered_policy_rule_public_ids: &[Uuid],
        ) -> Result<(), PolicyServiceError> {
            Err(self.error.clone())
        }

        async fn policy_rule_create(
            &self,
            _params: PolicyRuleCreateParams,
        ) -> Result<Uuid, PolicyServiceError> {
            Err(self.error.clone())
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
    async fn create_policy_set_trims_and_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = api_state(indexers.clone())?;
        let request = PolicySetCreateRequest {
            display_name: " Policies ".to_string(),
            scope: " global ".to_string(),
            enabled: Some(true),
        };

        let (status, Json(response)) = create_policy_set(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(response.policy_set_public_id, Uuid::nil());

        let calls = indexers.policy_calls.lock().expect("lock poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, "Policies");
        assert_eq!(calls[0].2, "global");
        assert_eq!(calls[0].3, Some(true));
        drop(calls);
        Ok(())
    }

    #[tokio::test]
    async fn create_policy_rule_parses_expiry() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = api_state(indexers.clone())?;
        let request = PolicyRuleCreateRequest {
            rule_type: " block_title_regex ".to_string(),
            match_field: " title ".to_string(),
            match_operator: " regex ".to_string(),
            sort_order: 10,
            match_value_text: Some("  sample  ".to_string()),
            match_value_int: None,
            match_value_uuid: None,
            value_set_items: Some(vec![PolicyRuleValueItemRequest {
                value_text: Some(" foo ".to_string()),
                value_int: None,
                value_bigint: None,
                value_uuid: None,
            }]),
            action: " drop_canonical ".to_string(),
            severity: " hard ".to_string(),
            is_case_insensitive: Some(true),
            rationale: Some(" test ".to_string()),
            expires_at: Some("2024-06-12T12:00:00Z".to_string()),
        };
        let policy_set_public_id = Uuid::new_v4();
        let (status, Json(response)) =
            create_policy_rule(Path(policy_set_public_id), State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(response.policy_rule_public_id, Uuid::nil());

        let calls = indexers.policy_rule_calls.lock().expect("lock poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].policy_set_public_id, policy_set_public_id);
        assert_eq!(calls[0].rule_type, "block_title_regex");
        assert_eq!(calls[0].match_field, "title");
        assert_eq!(calls[0].match_operator, "regex");
        assert_eq!(calls[0].match_value_text.as_deref(), Some("sample"));
        assert_eq!(calls[0].action, "drop_canonical");
        assert_eq!(calls[0].severity, "hard");
        assert!(calls[0].expires_at.is_some());
        drop(calls);
        Ok(())
    }

    #[tokio::test]
    async fn create_policy_rule_invalid_expiry_returns_bad_request() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = api_state(indexers)?;
        let request = PolicyRuleCreateRequest {
            rule_type: "block_title_regex".to_string(),
            match_field: "title".to_string(),
            match_operator: "regex".to_string(),
            sort_order: 10,
            match_value_text: Some("sample".to_string()),
            match_value_int: None,
            match_value_uuid: None,
            value_set_items: None,
            action: "drop_canonical".to_string(),
            severity: "hard".to_string(),
            is_case_insensitive: None,
            rationale: None,
            expires_at: Some("not-a-date".to_string()),
        };

        let err = create_policy_rule(Path(Uuid::new_v4()), State(state), Json(request))
            .await
            .err()
            .unwrap();
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let problem = parse_problem(response).await;
        assert_eq!(
            problem.detail.as_deref(),
            Some(POLICY_RULE_EXPIRES_AT_INVALID)
        );
        Ok(())
    }

    #[tokio::test]
    async fn policy_set_conflict_maps_conflict() -> Result<(), ApiError> {
        let indexers = Arc::new(ErrorIndexers {
            error: PolicyServiceError::new(PolicyServiceErrorKind::Conflict)
                .with_code("global_policy_set_exists"),
        });
        let state = api_state(indexers)?;
        let request = PolicySetCreateRequest {
            display_name: "Policies".to_string(),
            scope: "global".to_string(),
            enabled: Some(true),
        };

        let err = create_policy_set(State(state), Json(request))
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
}
