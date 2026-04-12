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
    RoutingPolicyCreateRequest, RoutingPolicyDetailResponse, RoutingPolicyListResponse,
    RoutingPolicyParamSetRequest, RoutingPolicyResponse, RoutingPolicySecretBindRequest,
};

const ROUTING_POLICY_CREATE_FAILED: &str = "failed to create routing policy";
const ROUTING_POLICY_GET_FAILED: &str = "failed to fetch routing policy";
const ROUTING_POLICY_LIST_FAILED: &str = "failed to fetch routing policies";
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

pub(crate) async fn list_routing_policies(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<RoutingPolicyListResponse>, ApiError> {
    let routing_policies = state
        .indexers
        .routing_policy_list(SYSTEM_ACTOR_PUBLIC_ID)
        .await
        .map_err(|err| {
            map_routing_policy_error("routing_policy_list", ROUTING_POLICY_LIST_FAILED, &err)
        })?;

    Ok(Json(RoutingPolicyListResponse { routing_policies }))
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
    use crate::app::indexers::RoutingPolicyServiceErrorKind;
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{RoutingPolicyListItemResponse, RoutingPolicyParameterResponse};
    use axum::response::IntoResponse;
    use std::sync::Arc;

    #[tokio::test]
    async fn create_routing_policy_trims_values_and_returns_payload() -> Result<(), ApiError> {
        let routing_policy_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.routing_policy_create_result.lock().expect("lock") =
            Some(Ok(routing_policy_public_id));
        let state = indexer_test_state(indexers.clone())?;

        let request = RoutingPolicyCreateRequest {
            display_name: " Proxy ".to_string(),
            mode: " direct ".to_string(),
        };

        let (status, Json(response)) = create_routing_policy(State(state), Json(request)).await?;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(response.routing_policy_public_id, routing_policy_public_id);
        assert_eq!(response.display_name, "Proxy");
        assert_eq!(response.mode, "direct");
        assert_eq!(
            indexers
                .routing_policy_create_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                "Proxy".to_string(),
                "direct".to_string()
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_routing_policy_returns_payload() -> Result<(), ApiError> {
        let routing_policy_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.routing_policy_get_result.lock().expect("lock") =
            Some(Ok(RoutingPolicyDetailResponse {
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
            }));
        let state = indexer_test_state(indexers.clone())?;

        let Json(response) =
            get_routing_policy(State(state), Path(routing_policy_public_id)).await?;
        assert_eq!(response.routing_policy_public_id, routing_policy_public_id);
        assert_eq!(response.parameters.len(), 1);
        assert_eq!(
            indexers
                .routing_policy_get_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(SYSTEM_ACTOR_PUBLIC_ID, routing_policy_public_id)]
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_routing_policies_returns_payload() -> Result<(), ApiError> {
        let routing_policy_public_id = Uuid::new_v4();
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.routing_policy_list_result.lock().expect("lock") =
            Some(Ok(vec![RoutingPolicyListItemResponse {
                routing_policy_public_id,
                display_name: "Proxy".to_string(),
                mode: "direct".to_string(),
                rate_limit_policy_public_id: Some(Uuid::new_v4()),
                rate_limit_display_name: Some("Proxy Budget".to_string()),
                parameter_count: 2,
                secret_binding_count: 1,
            }]));
        let state = indexer_test_state(indexers)?;

        let Json(response) = list_routing_policies(State(state)).await?;
        assert_eq!(response.routing_policies.len(), 1);
        assert_eq!(
            response.routing_policies[0].routing_policy_public_id,
            routing_policy_public_id
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_routing_policy_param_trims_values_and_returns_no_content() -> Result<(), ApiError>
    {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
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
        assert_eq!(
            indexers
                .routing_policy_set_param_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                routing_policy_public_id,
                "proxy_host".to_string(),
                Some("localhost".to_string()),
                None,
                None,
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn bind_routing_policy_secret_returns_no_content() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        let state = indexer_test_state(indexers.clone())?;
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
        assert_eq!(
            indexers
                .routing_policy_bind_secret_calls
                .lock()
                .expect("lock")
                .as_slice(),
            &[(
                SYSTEM_ACTOR_PUBLIC_ID,
                routing_policy_public_id,
                "http_proxy_auth".to_string(),
                secret_public_id,
            )]
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_routing_policy_conflict_maps_conflict() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.routing_policy_create_result.lock().expect("lock") = Some(Err(
            RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::Conflict)
                .with_code("display_name_already_exists"),
        ));
        let state = indexer_test_state(indexers)?;

        let request = RoutingPolicyCreateRequest {
            display_name: "Routing".to_string(),
            mode: "direct".to_string(),
        };

        let response = create_routing_policy(State(state), Json(request))
            .await
            .expect_err("conflict should fail")
            .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let problem = parse_problem(response).await;
        let context = problem.context.unwrap_or_default();
        assert!(context.iter().any(|field| field.name == "error_code"));
        Ok(())
    }

    #[tokio::test]
    async fn bind_routing_policy_secret_not_found_maps_problem_context() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers
            .routing_policy_bind_secret_error
            .lock()
            .expect("lock") = Some(
            RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
                .with_code("secret_not_found"),
        );
        let state = indexer_test_state(indexers)?;

        let response = bind_routing_policy_secret(
            State(state),
            Path(Uuid::new_v4()),
            Json(RoutingPolicySecretBindRequest {
                param_key: "http_proxy_auth".to_string(),
                secret_public_id: Uuid::new_v4(),
            }),
        )
        .await
        .expect_err("missing secret should fail")
        .into_response();
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
        let indexers = Arc::new(RecordingIndexers::default());
        *indexers.routing_policy_get_result.lock().expect("lock") = Some(Err(
            RoutingPolicyServiceError::new(RoutingPolicyServiceErrorKind::NotFound)
                .with_code("routing_policy_not_found"),
        ));
        let state = indexer_test_state(indexers)?;

        let response = get_routing_policy(State(state), Path(Uuid::new_v4()))
            .await
            .expect_err("missing policy should fail")
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(ROUTING_POLICY_GET_FAILED));
        Ok(())
    }
}
