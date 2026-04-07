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
    PolicyRuleValueItemRequest, PolicySetCreateRequest, PolicySetListResponse,
    PolicySetReorderRequest, PolicySetResponse, PolicySetUpdateRequest,
};

const POLICY_SET_LIST_FAILED: &str = "failed to list policy sets";
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

pub(crate) async fn list_policy_sets(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<PolicySetListResponse>, ApiError> {
    let policy_sets = state
        .indexers
        .policy_set_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| map_policy_error("policy_set_list", POLICY_SET_LIST_FAILED, &err))?;

    Ok(Json(PolicySetListResponse { policy_sets }))
}

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
    use crate::app::indexers::{PolicyServiceError, PolicyServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{PolicyRuleListItemResponse, PolicySetListItemResponse};
    use axum::response::IntoResponse;
    use std::sync::Arc;
    use uuid::Uuid;

    #[tokio::test]
    async fn create_policy_set_trims_and_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let request = PolicySetCreateRequest {
            display_name: " Policies ".to_string(),
            scope: " global ".to_string(),
            enabled: Some(true),
        };

        let (status, Json(response)) = create_policy_set(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_ne!(response.policy_set_public_id, Uuid::nil());

        let calls = indexers
            .policy_set_create_calls
            .lock()
            .expect("lock poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, "Policies");
        assert_eq!(calls[0].2, "global");
        assert_eq!(calls[0].3, Some(true));
        drop(calls);
        Ok(())
    }

    #[tokio::test]
    async fn list_policy_sets_returns_payload() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let policy_set_public_id = Uuid::new_v4();
        let policy_rule_public_id = Uuid::new_v4();
        indexers
            .policy_set_list_items
            .lock()
            .expect("lock poisoned")
            .push(PolicySetListItemResponse {
                policy_set_public_id,
                display_name: "Default".to_string(),
                scope: "global".to_string(),
                is_enabled: true,
                user_public_id: None,
                rules: vec![PolicyRuleListItemResponse {
                    policy_rule_public_id,
                    rule_type: "block_release_group".to_string(),
                    match_field: "release_group".to_string(),
                    match_operator: "equals".to_string(),
                    sort_order: 1,
                    match_value_text: Some("group".to_string()),
                    match_value_int: None,
                    match_value_uuid: None,
                    action: "drop".to_string(),
                    severity: "hard".to_string(),
                    is_case_insensitive: false,
                    rationale: Some("operator".to_string()),
                    expires_at: None,
                    is_disabled: false,
                }],
            });
        let state = indexer_test_state(indexers.clone())?;

        let Json(response) = list_policy_sets(State(state)).await?;
        assert_eq!(response.policy_sets.len(), 1);
        assert_eq!(
            response.policy_sets[0].policy_set_public_id,
            policy_set_public_id
        );
        assert_eq!(
            response.policy_sets[0].rules[0].policy_rule_public_id,
            policy_rule_public_id
        );

        let calls = indexers
            .policy_set_list_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(calls.as_slice(), &[SYSTEM_ACTOR_PUBLIC_ID]);
        Ok(())
    }

    #[tokio::test]
    async fn update_policy_set_trims_optional_display_name() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_set_public_id = Uuid::new_v4();
        let request = PolicySetUpdateRequest {
            display_name: Some("  Movies  ".to_string()),
        };

        let Json(response) =
            update_policy_set(Path(policy_set_public_id), State(state), Json(request)).await?;
        assert_eq!(response.policy_set_public_id, policy_set_public_id);

        let calls = indexers
            .policy_set_update_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, SYSTEM_ACTOR_PUBLIC_ID);
        assert_eq!(calls[0].1, policy_set_public_id);
        assert_eq!(calls[0].2.as_deref(), Some("Movies"));
        Ok(())
    }

    #[tokio::test]
    async fn enable_policy_set_records_identifier() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_set_public_id = Uuid::new_v4();

        let status = enable_policy_set(Path(policy_set_public_id), State(state)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_set_enable_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, policy_set_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn disable_policy_set_records_identifier() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_set_public_id = Uuid::new_v4();

        let status = disable_policy_set(Path(policy_set_public_id), State(state)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_set_disable_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, policy_set_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reorder_policy_sets_records_order() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let request = PolicySetReorderRequest {
            ordered_policy_set_public_ids: vec![first, second],
        };

        let status = reorder_policy_sets(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_set_reorder_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, vec![first, second])]
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_policy_rule_parses_expiry() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
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

        let calls = indexers
            .policy_rule_create_calls
            .lock()
            .expect("lock poisoned");
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
        let state = indexer_test_state(indexers)?;
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
    async fn enable_policy_rule_records_identifier() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_rule_public_id = Uuid::new_v4();

        let status = enable_policy_rule(Path(policy_rule_public_id), State(state)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_rule_enable_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, policy_rule_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn disable_policy_rule_records_identifier() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_rule_public_id = Uuid::new_v4();

        let status = disable_policy_rule(Path(policy_rule_public_id), State(state)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_rule_disable_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, policy_rule_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn reorder_policy_rules_records_scope_and_order() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
        let policy_set_public_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let request = PolicyRuleReorderRequest {
            ordered_policy_rule_public_ids: vec![first, second],
        };

        let status =
            reorder_policy_rules(Path(policy_set_public_id), State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let calls = indexers
            .policy_rule_reorder_calls
            .lock()
            .expect("lock poisoned")
            .clone();
        assert_eq!(
            calls.as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                policy_set_public_id,
                vec![first, second]
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn policy_set_conflict_maps_conflict() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .policy_set_create_result
            .lock()
            .expect("lock poisoned") = Some(Err(PolicyServiceError::new(
            PolicyServiceErrorKind::Conflict,
        )
        .with_code("global_policy_set_exists")));
        let state = indexer_test_state(indexers)?;
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
