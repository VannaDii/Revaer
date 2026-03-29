//! RSS management endpoints for indexer instances.
//!
//! # Design
//! - Delegate RSS status and seen-item operations to the injected indexer facade.
//! - Reuse the indexer instance error mapping for stable problem responses.
//! - Keep operator flows explicit: fetch status, update interval, inspect history, mark seen.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::indexers::{
    IndexerRssSeenListParams, IndexerRssSeenMarkParams, IndexerRssSubscriptionParams,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::instances::map_instance_error;
use crate::http::handlers::indexers::normalization::trim_and_filter_empty;
use crate::models::{
    IndexerRssSeenItemsResponse, IndexerRssSeenMarkRequest, IndexerRssSeenMarkResponse,
    IndexerRssSubscriptionResponse, IndexerRssSubscriptionUpdateRequest,
};

const RSS_SUBSCRIPTION_GET_FAILED: &str = "failed to fetch rss subscription";
const RSS_SUBSCRIPTION_SET_FAILED: &str = "failed to update rss subscription";
const RSS_ITEMS_LIST_FAILED: &str = "failed to list rss items";
const RSS_ITEM_MARK_FAILED: &str = "failed to mark rss item seen";

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub(crate) struct RssItemsQuery {
    pub limit: Option<i32>,
}

pub(crate) async fn get_indexer_rss_subscription(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
) -> Result<Json<IndexerRssSubscriptionResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_rss_subscription_get(SYSTEM_ACTOR_PUBLIC_ID, indexer_instance_public_id)
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_rss_subscription_get",
                RSS_SUBSCRIPTION_GET_FAILED,
                &err,
            )
        })?;

    Ok(Json(response))
}

pub(crate) async fn put_indexer_rss_subscription(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerRssSubscriptionUpdateRequest>,
) -> Result<Json<IndexerRssSubscriptionResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_rss_subscription_set(IndexerRssSubscriptionParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            is_enabled: request.is_enabled,
            interval_seconds: request.interval_seconds,
        })
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_rss_subscription_set",
                RSS_SUBSCRIPTION_SET_FAILED,
                &err,
            )
        })?;

    Ok(Json(response))
}

pub(crate) async fn get_indexer_rss_items(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Query(query): Query<RssItemsQuery>,
) -> Result<Json<IndexerRssSeenItemsResponse>, ApiError> {
    let items = state
        .indexers
        .indexer_rss_seen_list(IndexerRssSeenListParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            limit: query.limit,
        })
        .await
        .map_err(|err| map_instance_error("indexer_rss_seen_list", RSS_ITEMS_LIST_FAILED, &err))?;

    Ok(Json(IndexerRssSeenItemsResponse { items }))
}

pub(crate) async fn mark_indexer_rss_item_seen(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Json(request): Json<IndexerRssSeenMarkRequest>,
) -> Result<Json<IndexerRssSeenMarkResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_rss_seen_mark(IndexerRssSeenMarkParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            item_guid: trim_and_filter_empty(request.item_guid.as_deref()),
            infohash_v1: trim_and_filter_empty(request.infohash_v1.as_deref()),
            infohash_v2: trim_and_filter_empty(request.infohash_v2.as_deref()),
            magnet_hash: trim_and_filter_empty(request.magnet_hash.as_deref()),
        })
        .await
        .map_err(|err| map_instance_error("indexer_rss_seen_mark", RSS_ITEM_MARK_FAILED, &err))?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::{IndexerInstanceServiceError, IndexerInstanceServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{IndexerRssSeenItemResponse, IndexerRssSubscriptionResponse};
    use axum::response::IntoResponse;
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn get_indexer_rss_subscription_returns_payload() {
        let indexers = RecordingIndexers::default();
        let response = IndexerRssSubscriptionResponse {
            indexer_instance_public_id: Uuid::new_v4(),
            instance_status: "enabled".to_string(),
            rss_setting_status: "enabled".to_string(),
            subscription_status: "enabled".to_string(),
            interval_seconds: 900,
            last_polled_at: None,
            next_poll_at: None,
            backoff_seconds: None,
            last_error_class: None,
        };
        *indexers.rss_subscription_response.lock().expect("lock") = Some(response.clone());
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_indexer_rss_subscription(State(state), Path(Uuid::new_v4()))
            .await
            .expect("ok");
        assert_eq!(body, response);
    }

    #[tokio::test]
    async fn get_indexer_rss_items_returns_payload() {
        let indexers = RecordingIndexers::default();
        *indexers.rss_seen_items_response.lock().expect("lock") =
            Some(vec![IndexerRssSeenItemResponse {
                item_guid: Some("guid".to_string()),
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
                first_seen_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            }]);
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_indexer_rss_items(
            State(state),
            Path(Uuid::new_v4()),
            Query(RssItemsQuery { limit: Some(10) }),
        )
        .await
        .expect("ok");
        assert_eq!(body.items.len(), 1);
        assert_eq!(body.items[0].item_guid.as_deref(), Some("guid"));
    }

    #[tokio::test]
    async fn mark_indexer_rss_item_seen_returns_payload() {
        let indexers = RecordingIndexers::default();
        *indexers.rss_seen_mark_response.lock().expect("lock") = Some(IndexerRssSeenMarkResponse {
            item: IndexerRssSeenItemResponse {
                item_guid: Some("guid".to_string()),
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
                first_seen_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
            },
            inserted: true,
        });
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = mark_indexer_rss_item_seen(
            State(state),
            Path(Uuid::new_v4()),
            Json(IndexerRssSeenMarkRequest {
                item_guid: Some("guid".to_string()),
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
            }),
        )
        .await
        .expect("ok");
        assert!(body.inserted);
        assert_eq!(body.item.item_guid.as_deref(), Some("guid"));
    }

    #[tokio::test]
    async fn put_indexer_rss_subscription_maps_conflict() {
        let indexers = RecordingIndexers::default();
        *indexers.rss_subscription_error.lock().expect("lock") = Some(
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Conflict),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = put_indexer_rss_subscription(
            State(state),
            Path(Uuid::new_v4()),
            Json(IndexerRssSubscriptionUpdateRequest {
                is_enabled: true,
                interval_seconds: Some(900),
            }),
        )
        .await
        .expect_err("conflict")
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);

        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(RSS_SUBSCRIPTION_SET_FAILED));
    }
}
