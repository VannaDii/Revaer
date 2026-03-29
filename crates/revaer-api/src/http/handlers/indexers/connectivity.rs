//! Connectivity and reputation endpoints for indexer instances.
//!
//! # Design
//! - Delegate connectivity/reputation reads to the injected indexer facade.
//! - Reuse the shared instance error mapper for stable problem responses.
//! - Keep the surface read-only; remediation remains explicit via existing CF reset actions.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::indexers::{IndexerHealthEventListParams, IndexerSourceReputationListParams};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::http::handlers::indexers::instances::map_instance_error;
use crate::models::{
    IndexerConnectivityProfileResponse, IndexerHealthEventListResponse,
    IndexerSourceReputationListResponse,
};

const CONNECTIVITY_PROFILE_GET_FAILED: &str = "failed to fetch connectivity profile";
const HEALTH_EVENT_LIST_FAILED: &str = "failed to list health events";
const SOURCE_REPUTATION_LIST_FAILED: &str = "failed to list source reputation";

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ReputationQuery {
    pub window_key: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct HealthEventsQuery {
    pub limit: Option<i32>,
}

pub(crate) async fn get_indexer_connectivity_profile(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
) -> Result<Json<IndexerConnectivityProfileResponse>, ApiError> {
    let response = state
        .indexers
        .indexer_connectivity_profile_get(SYSTEM_ACTOR_PUBLIC_ID, indexer_instance_public_id)
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_connectivity_profile_get",
                CONNECTIVITY_PROFILE_GET_FAILED,
                &err,
            )
        })?;

    Ok(Json(response))
}

pub(crate) async fn get_indexer_source_reputation(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Query(query): Query<ReputationQuery>,
) -> Result<Json<IndexerSourceReputationListResponse>, ApiError> {
    let items = state
        .indexers
        .indexer_source_reputation_list(IndexerSourceReputationListParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            window_key: query.window_key.as_deref(),
            limit: query.limit,
        })
        .await
        .map_err(|err| {
            map_instance_error(
                "indexer_source_reputation_list",
                SOURCE_REPUTATION_LIST_FAILED,
                &err,
            )
        })?;

    Ok(Json(IndexerSourceReputationListResponse { items }))
}

pub(crate) async fn get_indexer_health_events(
    State(state): State<Arc<ApiState>>,
    Path(indexer_instance_public_id): Path<Uuid>,
    Query(query): Query<HealthEventsQuery>,
) -> Result<Json<IndexerHealthEventListResponse>, ApiError> {
    let items = state
        .indexers
        .indexer_health_event_list(IndexerHealthEventListParams {
            actor_user_public_id: SYSTEM_ACTOR_PUBLIC_ID,
            indexer_instance_public_id,
            limit: query.limit,
        })
        .await
        .map_err(|err| {
            map_instance_error("indexer_health_event_list", HEALTH_EVENT_LIST_FAILED, &err)
        })?;

    Ok(Json(IndexerHealthEventListResponse { items }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::{IndexerInstanceServiceError, IndexerInstanceServiceErrorKind};
    use crate::http::handlers::indexers::test_support::{
        RecordingIndexers, indexer_test_state, parse_problem,
    };
    use crate::models::{IndexerHealthEventResponse, IndexerSourceReputationResponse};
    use axum::response::IntoResponse;
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    #[tokio::test]
    async fn get_connectivity_profile_returns_payload() {
        let indexers = RecordingIndexers::default();
        *indexers.connectivity_profile_response.lock().expect("lock") =
            Some(IndexerConnectivityProfileResponse {
                profile_exists: true,
                status: Some("failing".to_string()),
                error_class: Some("cf_challenge".to_string()),
                latency_p50_ms: Some(1200),
                latency_p95_ms: Some(3500),
                success_rate_1h: Some(0.85),
                success_rate_24h: Some(0.91),
                last_checked_at: Some(Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap()),
            });
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_indexer_connectivity_profile(State(state), Path(Uuid::new_v4()))
            .await
            .expect("ok");
        assert!(body.profile_exists);
        assert_eq!(body.status.as_deref(), Some("failing"));
    }

    #[tokio::test]
    async fn get_source_reputation_returns_payload() {
        let indexers = RecordingIndexers::default();
        *indexers.source_reputation_response.lock().expect("lock") =
            Some(vec![IndexerSourceReputationResponse {
                window_key: "1h".to_string(),
                window_start: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
                request_success_rate: 0.75,
                acquisition_success_rate: 0.5,
                fake_rate: 0.1,
                dmca_rate: 0.05,
                request_count: 40,
                request_success_count: 30,
                acquisition_count: 10,
                acquisition_success_count: 5,
                min_samples: 10,
                computed_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 5, 0).unwrap(),
            }]);
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_indexer_source_reputation(
            State(state),
            Path(Uuid::new_v4()),
            Query(ReputationQuery {
                window_key: Some("1h".to_string()),
                limit: Some(10),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(body.items.len(), 1);
        assert_eq!(body.items[0].window_key, "1h");
    }

    #[tokio::test]
    async fn get_source_reputation_maps_conflict() {
        let indexers = RecordingIndexers::default();
        *indexers.source_reputation_error.lock().expect("lock") = Some(
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Conflict),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = get_indexer_source_reputation(
            State(state),
            Path(Uuid::new_v4()),
            Query(ReputationQuery {
                window_key: Some("1h".to_string()),
                limit: Some(10),
            }),
        )
        .await
        .expect_err("conflict")
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);

        let problem = parse_problem(response).await;
        assert_eq!(
            problem.detail.as_deref(),
            Some(SOURCE_REPUTATION_LIST_FAILED)
        );
    }

    #[tokio::test]
    async fn get_health_events_returns_payload() {
        let indexers = RecordingIndexers::default();
        *indexers.health_event_response.lock().expect("lock") =
            Some(vec![IndexerHealthEventResponse {
                occurred_at: Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap(),
                event_type: "identity_conflict".to_string(),
                latency_ms: Some(1250),
                http_status: Some(503),
                error_class: Some("cf_challenge".to_string()),
                detail: Some("challenge observed".to_string()),
            }]);
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let Json(body) = get_indexer_health_events(
            State(state),
            Path(Uuid::new_v4()),
            Query(HealthEventsQuery { limit: Some(10) }),
        )
        .await
        .expect("ok");
        assert_eq!(body.items.len(), 1);
        assert_eq!(body.items[0].event_type, "identity_conflict");
    }

    #[tokio::test]
    async fn get_health_events_maps_conflict() {
        let indexers = RecordingIndexers::default();
        *indexers.health_event_error.lock().expect("lock") = Some(
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Conflict),
        );
        let state = indexer_test_state(Arc::new(indexers)).expect("state");

        let response = get_indexer_health_events(
            State(state),
            Path(Uuid::new_v4()),
            Query(HealthEventsQuery { limit: Some(10) }),
        )
        .await
        .expect_err("conflict")
        .into_response();
        assert_eq!(response.status(), axum::http::StatusCode::CONFLICT);

        let problem = parse_problem(response).await;
        assert_eq!(problem.detail.as_deref(), Some(HEALTH_EVENT_LIST_FAILED));
    }
}
