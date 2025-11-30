//! Health and diagnostics endpoints.

use std::sync::Arc;

use axum::{Json, body::Body, extract::State, http::StatusCode, response::Response};
use revaer_config::AppMode;
use revaer_telemetry::{build_sha, record_app_mode};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::http::errors::ApiError;
use crate::state::ApiState;
#[derive(Serialize)]
pub(crate) struct HealthComponent {
    pub(crate) status: &'static str,
    pub(crate) revision: Option<i64>,
}

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) status: &'static str,
    pub(crate) mode: AppMode,
    pub(crate) database: HealthComponent,
}

#[derive(Serialize)]
pub(crate) struct FullHealthResponse {
    pub(crate) status: &'static str,
    pub(crate) mode: AppMode,
    pub(crate) revision: i64,
    pub(crate) build: String,
    pub(crate) degraded: Vec<String>,
    pub(crate) metrics: HealthMetricsResponse,
    pub(crate) torrent: TorrentHealthSnapshot,
}

#[derive(Serialize)]
pub(crate) struct HealthMetricsResponse {
    pub(crate) config_watch_latency_ms: i64,
    pub(crate) config_apply_latency_ms: i64,
    pub(crate) config_update_failures_total: u64,
    pub(crate) config_watch_slow_total: u64,
    pub(crate) guardrail_violations_total: u64,
    pub(crate) rate_limit_throttled_total: u64,
}

#[derive(Serialize)]
pub(crate) struct TorrentHealthSnapshot {
    pub(crate) active: i64,
    pub(crate) queue_depth: i64,
}

#[derive(Serialize)]
pub(crate) struct DashboardResponse {
    pub(crate) download_bps: u64,
    pub(crate) upload_bps: u64,
    pub(crate) active: u32,
    pub(crate) paused: u32,
    pub(crate) completed: u32,
    pub(crate) disk_total_gb: u32,
    pub(crate) disk_used_gb: u32,
}

pub(crate) async fn health(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<HealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.remove_degraded_component("database");
            record_app_mode(snapshot.app_profile.mode.as_str());
            Ok(Json(HealthResponse {
                status: "ok",
                mode: snapshot.app_profile.mode.clone(),
                database: HealthComponent {
                    status: "ok",
                    revision: Some(snapshot.revision),
                },
            }))
        }
        Err(err) => {
            state.add_degraded_component("database");
            warn!(error = %err, "health check failed to reach database");
            Err(ApiError::service_unavailable(
                "database is currently unavailable",
            ))
        }
    }
}

pub(crate) async fn health_full(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FullHealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.remove_degraded_component("database");
            record_app_mode(snapshot.app_profile.mode.as_str());
            state.update_torrent_metrics().await;
            let metrics_snapshot = state.telemetry.snapshot();
            let metrics = HealthMetricsResponse {
                config_watch_latency_ms: metrics_snapshot.config_watch_latency_ms,
                config_apply_latency_ms: metrics_snapshot.config_apply_latency_ms,
                config_update_failures_total: metrics_snapshot.config_update_failures_total,
                config_watch_slow_total: metrics_snapshot.config_watch_slow_total,
                guardrail_violations_total: metrics_snapshot.guardrail_violations_total,
                rate_limit_throttled_total: metrics_snapshot.rate_limit_throttled_total,
            };
            let torrent = TorrentHealthSnapshot {
                active: metrics_snapshot.active_torrents,
                queue_depth: metrics_snapshot.queue_depth,
            };
            let degraded = state.current_health_degraded();
            let status = if degraded.is_empty() {
                "ok"
            } else {
                "degraded"
            };
            Ok(Json(FullHealthResponse {
                status,
                mode: snapshot.app_profile.mode.clone(),
                revision: snapshot.revision,
                build: build_sha().to_string(),
                degraded,
                metrics,
                torrent,
            }))
        }
        Err(err) => {
            state.add_degraded_component("database");
            warn!(error = %err, "full health check failed to reach database");
            Err(ApiError::service_unavailable(
                "database is currently unavailable",
            ))
        }
    }
}

pub(crate) async fn dashboard(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<DashboardResponse>, ApiError> {
    info!("dashboard request");
    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile for dashboard");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    Ok(Json(DashboardResponse {
        download_bps: 0,
        upload_bps: 0,
        active: 0,
        paused: 0,
        completed: 0,
        disk_total_gb: 0,
        disk_used_gb: 0,
    }))
}

pub(crate) async fn metrics(State(state): State<Arc<ApiState>>) -> Result<Response, ApiError> {
    match state.telemetry.render() {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )
            .body(Body::from(body))
            .map_err(|err| {
                error!(error = %err, "failed to build metrics response");
                ApiError::internal("failed to build metrics response")
            }),
        Err(err) => {
            error!(error = %err, "failed to render metrics");
            Err(ApiError::internal("failed to render metrics"))
        }
    }
}
