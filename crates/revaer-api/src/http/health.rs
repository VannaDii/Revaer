//! Health and diagnostics endpoints.

use std::sync::Arc;

use axum::{Json, body::Body, extract::State, http::StatusCode, response::Response};
use revaer_telemetry::{build_sha, record_app_mode};
use tracing::{error, info, warn};

use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::models::{
    DashboardResponse, FullHealthResponse, HealthComponentResponse, HealthMetricsResponse,
    HealthResponse, TorrentHealthResponse,
};

pub(crate) async fn health(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<HealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.remove_degraded_component("database");
            record_app_mode(snapshot.app_profile.mode.as_str());
            Ok(Json(HealthResponse {
                status: "ok".to_string(),
                mode: snapshot.app_profile.mode.as_str().to_string(),
                database: HealthComponentResponse {
                    status: "ok".to_string(),
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
            let torrent = TorrentHealthResponse {
                active: metrics_snapshot.active_torrents,
                queue_depth: metrics_snapshot.queue_depth,
            };
            let degraded = state.current_health_degraded();
            let status = if degraded.is_empty() {
                "ok".to_string()
            } else {
                "degraded".to_string()
            };
            Ok(Json(FullHealthResponse {
                status,
                mode: snapshot.app_profile.mode.as_str().to_string(),
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
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        EngineProfile, FsPolicy, SettingsChangeset, SetupToken, TelemetryConfig,
        engine_profile::{AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig},
        normalize_engine_profile,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
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

        fn maybe_fail<T>(&self, operation: &'static str, value: T) -> ConfigResult<T> {
            if self.fail {
                Err(ConfigError::Io {
                    operation,
                    source: std::io::Error::other("stubbed config failure"),
                })
            } else {
                Ok(value)
            }
        }
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            self.maybe_fail("health.get_app_profile", self.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(
            &self,
            _ttl: Duration,
            _issued_by: &str,
        ) -> ConfigResult<SetupToken> {
            self.maybe_fail(
                "health.issue_setup_token",
                SetupToken {
                    plaintext: "token".into(),
                    expires_at: Utc::now(),
                },
            )
        }

        async fn validate_setup_token(&self, _token: &str) -> ConfigResult<()> {
            self.maybe_fail("health.validate_setup_token", ())
        }

        async fn consume_setup_token(&self, _token: &str) -> ConfigResult<()> {
            self.maybe_fail("health.consume_setup_token", ())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            self.maybe_fail("health.apply_changeset", ())?;
            Err(ConfigError::Io {
                operation: "health.apply_changeset",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            self.maybe_fail("health.snapshot", self.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> ConfigResult<Option<ApiKeyAuth>> {
            self.maybe_fail("health.authenticate_api_key", None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            self.maybe_fail("health.has_api_keys", true)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            self.maybe_fail("health.factory_reset", ())
        }
    }

    fn sample_snapshot(mode: AppMode) -> ConfigSnapshot {
        let bind_addr = std::net::IpAddr::from([127, 0, 0, 1]);
        let engine_profile = EngineProfile {
            id: Uuid::nil(),
            implementation: "stub".into(),
            listen_port: None,
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(1),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: AltSpeedConfig::default(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: ".server_root/resume".into(),
            download_root: ".server_root/downloads".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: IpFilterConfig::default(),
            peer_classes: PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        };
        ConfigSnapshot {
            revision: 7,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 3030,
                bind_addr,
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::nil(),
                library_root: ".server_root/library".into(),
                extract: false,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: Vec::new(),
                cleanup_drop: Vec::new(),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: Vec::new(),
            },
        }
    }

    #[tokio::test]
    async fn health_success_clears_degraded_component() -> Result<()> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::healthy(AppMode::Active));
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            None,
        ));
        state.add_degraded_component("database");

        let response = health(State(state.clone())).await?;
        assert_eq!(response.0.status, "ok");
        assert!(
            state.current_health_degraded().is_empty(),
            "database component should be cleared on success"
        );
        Ok(())
    }

    #[tokio::test]
    async fn health_failure_marks_database_degraded() -> Result<()> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::failing(AppMode::Active));
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            None,
        ));

        let err = health(State(state.clone()))
            .await
            .err()
            .ok_or_else(|| anyhow!("expected failure"))?;
        assert_eq!(err.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            state
                .current_health_degraded()
                .iter()
                .any(|component| component == "database"),
            "database component should be marked degraded on failure"
        );
        Ok(())
    }

    #[tokio::test]
    async fn dashboard_maps_config_errors_to_internal() -> Result<()> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::failing(AppMode::Active));
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            None,
        ));

        let result = dashboard(State(state))
            .await
            .err()
            .ok_or_else(|| anyhow!("expected dashboard failure"))?;
        assert_eq!(result.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(result.kind(), crate::http::constants::PROBLEM_INTERNAL);
        Ok(())
    }
}
