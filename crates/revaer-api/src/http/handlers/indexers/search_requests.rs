//! Search request endpoints for indexers.
//!
//! # Design
//! - Delegate search request orchestration to the injected indexer facade.
//! - Keep messages constant and attach diagnostic context fields.
//! - REST search requests require API key authentication, enforced by the router/layer before
//!   these handlers run.

use std::sync::Arc;

use axum::{Json, extract::Extension, extract::Path, extract::State, http::StatusCode};
use uuid::Uuid;

use crate::app::indexers::{
    SearchRequestCreateParams, SearchRequestServiceError, SearchRequestServiceErrorKind,
};
use crate::app::state::ApiState;
use crate::http::auth::AuthContext;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::normalization::{
    normalize_required_str_field, trim_and_filter_empty,
};
use crate::models::{SearchRequestCreateRequest, SearchRequestCreateResponse};

const SEARCH_REQUEST_CREATE_FAILED: &str = "failed to create search request";
const SEARCH_REQUEST_CANCEL_FAILED: &str = "failed to cancel search request";
const QUERY_TYPE_REQUIRED: &str = "query_type is required";

#[tracing::instrument(name = "indexer.search.request.create", skip(state, auth, request))]
pub(crate) async fn create_search_request(
    State(state): State<Arc<ApiState>>,
    Extension(auth): Extension<AuthContext>,
    Json(request): Json<SearchRequestCreateRequest>,
) -> Result<(StatusCode, Json<SearchRequestCreateResponse>), ApiError> {
    let query_type = normalize_required_str_field(&request.query_type, QUERY_TYPE_REQUIRED)?;
    let query_text = request.query_text.trim();
    let torznab_mode = trim_and_filter_empty(request.torznab_mode.as_deref());
    let requested_media_domain_key =
        trim_and_filter_empty(request.requested_media_domain_key.as_deref());

    let params = SearchRequestCreateParams {
        actor_user_public_id: search_request_actor(&auth, torznab_mode),
        query_text,
        query_type,
        torznab_mode,
        requested_media_domain_key,
        page_size: request.page_size,
        search_profile_public_id: request.search_profile_public_id,
        request_policy_set_public_id: request.request_policy_set_public_id,
        season_number: request.season_number,
        episode_number: request.episode_number,
        identifier_types: request.identifier_types.as_deref(),
        identifier_values: request.identifier_values.as_deref(),
        torznab_cat_ids: request.torznab_cat_ids.as_deref(),
    };

    let response = match state.indexers.search_request_create(params).await {
        Ok(response) => {
            state
                .telemetry
                .inc_indexer_search_request("create", "success");
            response
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_search_request("create", "error");
            return Err(map_search_request_error(
                "search_request_create",
                SEARCH_REQUEST_CREATE_FAILED,
                &err,
            ));
        }
    };

    Ok((StatusCode::CREATED, Json(response)))
}

const fn search_request_actor(auth: &AuthContext, torznab_mode: Option<&str>) -> Option<Uuid> {
    match auth {
        AuthContext::ApiKey { .. } if torznab_mode.is_some() => None,
        AuthContext::ApiKey { .. } | AuthContext::Anonymous | AuthContext::SetupToken(_) => {
            Some(SYSTEM_ACTOR_PUBLIC_ID)
        }
    }
}

#[tracing::instrument(
    name = "indexer.search.request.cancel",
    skip(state),
    fields(search_request_public_id = %search_request_public_id)
)]
pub(crate) async fn cancel_search_request(
    State(state): State<Arc<ApiState>>,
    Path(search_request_public_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    match state
        .indexers
        .search_request_cancel(SYSTEM_ACTOR_PUBLIC_ID, search_request_public_id)
        .await
    {
        Ok(()) => {
            state
                .telemetry
                .inc_indexer_search_request("cancel", "success");
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_search_request("cancel", "error");
            return Err(map_search_request_error(
                "search_request_cancel",
                SEARCH_REQUEST_CANCEL_FAILED,
                &err,
            ));
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn map_search_request_error(
    operation: &'static str,
    detail: &'static str,
    err: &SearchRequestServiceError,
) -> ApiError {
    let mut api_error = match err.kind() {
        SearchRequestServiceErrorKind::Invalid => ApiError::bad_request(detail),
        SearchRequestServiceErrorKind::NotFound => ApiError::not_found(detail),
        SearchRequestServiceErrorKind::Unauthorized => ApiError::unauthorized(detail),
        SearchRequestServiceErrorKind::Storage => ApiError::internal(detail),
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
    use crate::http::auth::AuthContext;
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, SearchRequestCreateSnapshot, indexer_test_state, parse_problem,
    };
    use crate::models::ProblemDetails;
    use axum::response::IntoResponse;
    use std::sync::Arc;

    #[tokio::test]
    async fn create_search_request_trims_inputs() {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone())).expect("state");

        let request = SearchRequestCreateRequest {
            query_text: "  dune ".to_string(),
            query_type: " free_text ".to_string(),
            torznab_mode: Some(" tv ".to_string()),
            requested_media_domain_key: Some(" movies ".to_string()),
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let (status, _) = create_search_request(
            State(state),
            Extension(AuthContext::ApiKey {
                key_id: "demo".to_string(),
            }),
            Json(request),
        )
        .await
        .expect("create request");
        assert_eq!(status, StatusCode::CREATED);

        let snapshot = {
            let calls = indexers.search_request_calls.lock().expect("lock");
            calls.last().cloned().expect("snapshot")
        };
        assert_eq!(
            snapshot,
            SearchRequestCreateSnapshot {
                actor_user_public_id: None,
                query_text: "dune".to_string(),
                query_type: "free_text".to_string(),
                torznab_mode: Some("tv".to_string()),
                requested_media_domain_key: Some("movies".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn create_search_request_requires_query_type() {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let request = SearchRequestCreateRequest {
            query_text: String::new(),
            query_type: "   ".to_string(),
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let response = create_search_request(
            State(state),
            Extension(AuthContext::ApiKey {
                key_id: "demo".to_string(),
            }),
            Json(request),
        )
        .await
        .expect_err("bad request")
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let problem: ProblemDetails = parse_problem(response).await;
        assert_eq!(problem.title, "bad request");
        assert_eq!(problem.detail.as_deref(), Some(QUERY_TYPE_REQUIRED));
    }

    #[tokio::test]
    async fn cancel_search_request_maps_not_found() {
        let indexers = RecordingIndexers::default();
        *indexers.search_request_cancel_error.lock().expect("lock") = Some(
            SearchRequestServiceError::new(SearchRequestServiceErrorKind::NotFound),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = cancel_search_request(State(state), Path(Uuid::new_v4()))
            .await
            .expect_err("not found")
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let problem: ProblemDetails = parse_problem(response).await;
        assert_eq!(problem.title, "resource not found");
        assert_eq!(
            problem.detail.as_deref(),
            Some(SEARCH_REQUEST_CANCEL_FAILED)
        );
    }

    #[tokio::test]
    async fn create_search_request_uses_system_actor_for_anonymous_mode() {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone())).expect("state");

        let request = SearchRequestCreateRequest {
            query_text: "search".to_string(),
            query_type: "free_text".to_string(),
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let (status, _) = create_search_request(
            State(state),
            Extension(AuthContext::Anonymous),
            Json(request),
        )
        .await
        .expect("create request");
        assert_eq!(status, StatusCode::CREATED);

        let snapshot = {
            let calls = indexers.search_request_calls.lock().expect("lock");
            calls.last().cloned().expect("snapshot")
        };
        assert_eq!(snapshot.actor_user_public_id, Some(SYSTEM_ACTOR_PUBLIC_ID));
    }

    #[tokio::test]
    async fn create_search_request_uses_system_actor_for_api_key_free_text_mode() {
        let indexers = RecordingIndexers::default();
        let state = indexer_test_state(Arc::new(indexers.clone())).expect("state");

        let request = SearchRequestCreateRequest {
            query_text: "search".to_string(),
            query_type: "free_text".to_string(),
            torznab_mode: None,
            requested_media_domain_key: None,
            page_size: None,
            search_profile_public_id: None,
            request_policy_set_public_id: None,
            season_number: None,
            episode_number: None,
            identifier_types: None,
            identifier_values: None,
            torznab_cat_ids: None,
        };

        let (status, _) = create_search_request(
            State(state),
            Extension(AuthContext::ApiKey {
                key_id: "demo".to_string(),
            }),
            Json(request),
        )
        .await
        .expect("create request");
        assert_eq!(status, StatusCode::CREATED);

        let snapshot = {
            let calls = indexers.search_request_calls.lock().expect("lock");
            calls.last().cloned().expect("snapshot")
        };
        assert_eq!(snapshot.actor_user_public_id, Some(SYSTEM_ACTOR_PUBLIC_ID));
    }
}
