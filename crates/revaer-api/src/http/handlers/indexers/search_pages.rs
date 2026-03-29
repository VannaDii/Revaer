//! Search request page listing and fetch endpoints.
//!
//! # Design
//! - Delegate search page reads to the injected indexer facade.
//! - Surface stable RFC9457 errors with context fields for diagnostics.
//! - Keep messages constant and avoid leaking input values in responses.

use std::sync::Arc;

use axum::{Json, extract::Path, extract::State};
use uuid::Uuid;

use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::search_requests::map_search_request_error;
use crate::models::{SearchPageListResponse, SearchPageResponse};

const SEARCH_PAGE_LIST_FAILED: &str = "failed to list search pages";
const SEARCH_PAGE_FETCH_FAILED: &str = "failed to fetch search page";

#[tracing::instrument(
    name = "indexer.search.page.list",
    skip(state),
    fields(search_request_public_id = %search_request_public_id)
)]
pub(crate) async fn list_search_pages(
    State(state): State<Arc<ApiState>>,
    Path(search_request_public_id): Path<Uuid>,
) -> Result<Json<SearchPageListResponse>, ApiError> {
    let response = match state
        .indexers
        .search_page_list(SYSTEM_ACTOR_PUBLIC_ID, search_request_public_id)
        .await
    {
        Ok(response) => {
            state
                .telemetry
                .inc_indexer_search_request("page_list", "success");
            response
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_search_request("page_list", "error");
            return Err(map_search_request_error(
                "search_page_list",
                SEARCH_PAGE_LIST_FAILED,
                &err,
            ));
        }
    };

    Ok(Json(response))
}

#[tracing::instrument(
    name = "indexer.search.page.fetch",
    skip(state),
    fields(
        search_request_public_id = %search_request_public_id,
        page_number = page_number
    )
)]
pub(crate) async fn get_search_page(
    State(state): State<Arc<ApiState>>,
    Path((search_request_public_id, page_number)): Path<(Uuid, i32)>,
) -> Result<Json<SearchPageResponse>, ApiError> {
    let response = match state
        .indexers
        .search_page_fetch(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_request_public_id,
            page_number,
        )
        .await
    {
        Ok(response) => {
            state
                .telemetry
                .inc_indexer_search_request("page_fetch", "success");
            response
        }
        Err(err) => {
            state
                .telemetry
                .inc_indexer_search_request("page_fetch", "error");
            return Err(map_search_request_error(
                "search_page_fetch",
                SEARCH_PAGE_FETCH_FAILED,
                &err,
            ));
        }
    };

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::{SearchRequestServiceError, SearchRequestServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{
        SearchPageItemResponse, SearchPageSummaryResponse, SearchRequestExplainabilityResponse,
    };
    use axum::response::IntoResponse;
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn list_search_pages_returns_payload() {
        let indexers = RecordingIndexers::default();
        let pages = SearchPageListResponse {
            pages: vec![SearchPageSummaryResponse {
                page_number: 1,
                sealed_at: None,
                item_count: 0,
            }],
            explainability: SearchRequestExplainabilityResponse {
                zero_runnable_indexers: true,
                skipped_canceled_indexers: 0,
                skipped_failed_indexers: 0,
                blocked_results: 0,
                blocked_rule_public_ids: Vec::new(),
                rate_limited_indexers: 0,
                retrying_indexers: 0,
            },
        };
        *indexers.search_page_list_response.lock().expect("lock") = Some(pages.clone());
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(response) = list_search_pages(State(state), Path(Uuid::new_v4()))
            .await
            .expect("ok");
        assert_eq!(response, pages);
    }

    #[tokio::test]
    async fn get_search_page_returns_payload() {
        let indexers = RecordingIndexers::default();
        let response = SearchPageResponse {
            page_number: 2,
            sealed_at: Some(Utc.with_ymd_and_hms(2026, 2, 6, 0, 0, 0).unwrap()),
            item_count: 1,
            items: vec![SearchPageItemResponse {
                position: 1,
                canonical_torrent_public_id: Uuid::new_v4(),
                title_display: "Demo".to_string(),
                size_bytes: Some(42),
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
                canonical_torrent_source_public_id: None,
                indexer_instance_public_id: None,
                indexer_display_name: None,
                seeders: None,
                leechers: None,
                published_at: None,
                download_url: None,
                magnet_uri: None,
                details_url: None,
                tracker_name: None,
                tracker_category: None,
                tracker_subcategory: None,
            }],
        };
        *indexers.search_page_fetch_response.lock().expect("lock") = Some(response.clone());
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_search_page(State(state), Path((Uuid::new_v4(), 2)))
            .await
            .expect("ok");
        assert_eq!(body, response);
    }

    #[tokio::test]
    async fn list_search_pages_maps_not_found() {
        let indexers = RecordingIndexers::default();
        *indexers.search_page_list_error.lock().expect("lock") = Some(
            SearchRequestServiceError::new(SearchRequestServiceErrorKind::NotFound),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = list_search_pages(State(state), Path(Uuid::new_v4()))
            .await
            .expect_err("not found")
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

        let problem = parse_problem(response).await;
        assert_eq!(problem.title, "resource not found");
        assert_eq!(problem.detail.as_deref(), Some(SEARCH_PAGE_LIST_FAILED));
    }

    #[tokio::test]
    async fn get_search_page_maps_not_found() {
        let indexers = RecordingIndexers::default();
        *indexers.search_page_fetch_error.lock().expect("lock") = Some(
            SearchRequestServiceError::new(SearchRequestServiceErrorKind::NotFound),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = get_search_page(State(state), Path((Uuid::new_v4(), 1)))
            .await
            .expect_err("not found")
            .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

        let problem = parse_problem(response).await;
        assert_eq!(problem.title, "resource not found");
        assert_eq!(problem.detail.as_deref(), Some(SEARCH_PAGE_FETCH_FAILED));
    }
}
