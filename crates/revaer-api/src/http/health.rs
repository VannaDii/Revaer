//! Health and diagnostics endpoints.

use std::sync::Arc;

use axum::{Json, body::Body, extract::State, http::StatusCode, response::Response};
use revaer_config::AppMode;
use revaer_telemetry::{build_sha, record_app_mode};
use serde::Serialize;
use tracing::{error, info, warn};

use crate::app::state::ApiState;
use crate::http::errors::ApiError;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFacade;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use chrono::Utc;
    use revaer_config::{
        ApiKeyAuth, AppProfile, AppliedChanges, ConfigSnapshot, EngineProfile, FsPolicy,
        SettingsChangeset, SetupToken, normalize_engine_profile,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig {
        snapshot: ConfigSnapshot,
        fail: bool,
    }

    impl StubConfig {
        fn healthy(mode: AppMode) -> Self {
            Self {
                snapshot: sample_snapshot(mode),
                fail: false,
            }
        }

        fn failing(mode: AppMode) -> Self {
            Self {
                snapshot: sample_snapshot(mode),
                fail: true,
            }
        }

        fn maybe_fail<T>(&self, value: T) -> Result<T> {
            if self.fail {
                Err(anyhow!("config unavailable"))
            } else {
                Ok(value)
            }
        }
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            self.maybe_fail(self.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(&self, _ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            self.maybe_fail(SetupToken {
                plaintext: "token".into(),
                expires_at: Utc::now(),
            })
        }

        async fn validate_setup_token(&self, _token: &str) -> Result<()> {
            self.maybe_fail(())
        }

        async fn consume_setup_token(&self, _token: &str) -> Result<()> {
            self.maybe_fail(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            self.maybe_fail(())?;
            Err(anyhow!("not implemented"))
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            self.maybe_fail(self.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            self.maybe_fail(None)
        }
    }

    fn sample_snapshot(mode: AppMode) -> ConfigSnapshot {
        let engine_profile = EngineProfile {
            id: Uuid::nil(),
            implementation: "stub".into(),
            listen_port: None,
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(1),
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/tmp".into(),
            download_root: "/tmp/downloads".into(),
            tracker: json!([]),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
        };
        ConfigSnapshot {
            revision: 7,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode,
                version: 1,
                http_port: 3030,
                bind_addr: "127.0.0.1".parse().expect("bind addr"),
                telemetry: json!({}),
                features: json!({}),
                immutable_keys: json!([]),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::nil(),
                library_root: "/tmp/library".into(),
                extract: false,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: json!([]),
                cleanup_drop: json!([]),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: json!([]),
            },
        }
    }

    #[tokio::test]
    async fn health_success_clears_degraded_component() {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::healthy(AppMode::Active));
        let telemetry = Metrics::new().expect("metrics");
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(json!({})),
            EventBus::new(),
            None,
        ));
        state.add_degraded_component("database");

        let response = health(State(state.clone())).await.expect("health ok");
        assert_eq!(response.0.status, "ok");
        assert!(
            state.current_health_degraded().is_empty(),
            "database component should be cleared on success"
        );
    }

    #[tokio::test]
    async fn health_failure_marks_database_degraded() {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::failing(AppMode::Active));
        let telemetry = Metrics::new().expect("metrics");
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(json!({})),
            EventBus::new(),
            None,
        ));

        let Err(err) = health(State(state.clone())).await else {
            panic!("expected failure")
        };
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            state
                .current_health_degraded()
                .iter()
                .any(|component| component == "database"),
            "database component should be marked degraded on failure"
        );
    }

    #[tokio::test]
    async fn dashboard_maps_config_errors_to_internal() {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::failing(AppMode::Active));
        let telemetry = Metrics::new().expect("metrics");
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(json!({})),
            EventBus::new(),
            None,
        ));

        let Err(result) = dashboard(State(state)).await else {
            panic!("expected dashboard failure")
        };
        assert_eq!(result.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(result.kind, crate::http::constants::PROBLEM_INTERNAL);
    }
}
