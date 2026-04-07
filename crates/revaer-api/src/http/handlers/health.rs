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
    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot for dashboard");
        ApiError::internal("failed to load configuration snapshot")
    })?;
    record_app_mode(snapshot.app_profile.mode.as_str());

    Ok(Json(
        state
            .dashboard_snapshot(std::path::Path::new(&snapshot.fs_policy.library_root))
            .await,
    ))
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
    use crate::app::indexers::test_indexers;
    use crate::config::ConfigFacade;
    use crate::http::torrents::TorrentHandles;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use chrono::Utc;
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        EngineProfile, FsPolicy, SettingsChangeset, SetupToken, TelemetryConfig,
        engine_profile::{AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig},
        normalize_engine_profile,
        validate::default_local_networks,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use revaer_torrent_core::{
        AddTorrent, PeerSnapshot, RemoveTorrent, TorrentInspector, TorrentProgress, TorrentRates,
        TorrentResult, TorrentStatus, TorrentWorkflow,
    };
    use std::{path::PathBuf, sync::Arc as StdArc, time::Duration};
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

    #[derive(Clone, Default)]
    struct StubWorkflow;

    #[async_trait]
    impl TorrentWorkflow for StubWorkflow {
        async fn add_torrent(&self, _request: AddTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> TorrentResult<()> {
            Ok(())
        }
    }

    struct StubInspector {
        statuses: Vec<TorrentStatus>,
        fail_list: bool,
    }

    #[async_trait]
    impl TorrentInspector for StubInspector {
        async fn list(&self) -> TorrentResult<Vec<TorrentStatus>> {
            if self.fail_list {
                Err(revaer_torrent_core::TorrentError::Unsupported {
                    operation: "dashboard.list",
                })
            } else {
                Ok(self.statuses.clone())
            }
        }

        async fn get(&self, id: Uuid) -> TorrentResult<Option<TorrentStatus>> {
            Ok(self.statuses.iter().find(|status| status.id == id).cloned())
        }

        async fn peers(&self, _id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
            Ok(Vec::new())
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
                local_networks: default_local_networks(),
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

    fn state_with_handles(
        config: Arc<dyn ConfigFacade>,
        inspector: StubInspector,
    ) -> Result<Arc<ApiState>> {
        let telemetry = Metrics::new()?;
        let handles = TorrentHandles::new(StdArc::new(StubWorkflow), StdArc::new(inspector));
        Ok(Arc::new(ApiState::new(
            config,
            test_indexers(),
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            Some(handles),
        )))
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _result = std::fs::remove_dir_all(&self.path);
        }
    }

    fn temp_dir() -> Result<TestDir> {
        let base = std::path::PathBuf::from(".server_root");
        std::fs::create_dir_all(&base)?;
        let path = base.join(format!("revaer-health-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&path)?;
        Ok(TestDir { path })
    }

    #[tokio::test]
    async fn health_success_clears_degraded_component() -> Result<()> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig::healthy(AppMode::Active));
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config,
            test_indexers(),
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
            test_indexers(),
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
            test_indexers(),
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

    #[tokio::test]
    async fn dashboard_returns_live_torrent_metrics() -> Result<()> {
        let temp = temp_dir()?;
        let library_root = temp.path().join("library");
        std::fs::create_dir_all(&library_root)?;
        std::fs::write(library_root.join("artifact.bin"), vec![0_u8; 16])?;

        let mut snapshot = sample_snapshot(AppMode::Active);
        snapshot.fs_policy.library_root = library_root.to_string_lossy().into_owned();
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig {
            snapshot,
            fail: false,
        });
        let state = state_with_handles(
            config,
            StubInspector {
                statuses: vec![
                    TorrentStatus {
                        id: Uuid::new_v4(),
                        state: revaer_events::TorrentState::Downloading,
                        rates: TorrentRates {
                            download_bps: 128,
                            upload_bps: 64,
                            ratio: 0.5,
                        },
                        progress: TorrentProgress {
                            bytes_downloaded: 5,
                            bytes_total: 10,
                            eta_seconds: Some(5),
                        },
                        ..TorrentStatus::default()
                    },
                    TorrentStatus {
                        id: Uuid::new_v4(),
                        state: revaer_events::TorrentState::Stopped,
                        ..TorrentStatus::default()
                    },
                    TorrentStatus {
                        id: Uuid::new_v4(),
                        state: revaer_events::TorrentState::Completed,
                        rates: TorrentRates {
                            download_bps: 1,
                            upload_bps: 2,
                            ratio: 1.0,
                        },
                        progress: TorrentProgress {
                            bytes_downloaded: 10,
                            bytes_total: 10,
                            eta_seconds: None,
                        },
                        ..TorrentStatus::default()
                    },
                ],
                fail_list: false,
            },
        )?;

        let Json(body) = dashboard(State(state)).await?;
        assert_eq!(body.download_bps, 129);
        assert_eq!(body.upload_bps, 66);
        assert_eq!(body.active, 1);
        assert_eq!(body.paused, 1);
        assert_eq!(body.completed, 1);
        assert!(body.disk_total_gb >= body.disk_used_gb);
        Ok(())
    }

    #[tokio::test]
    async fn dashboard_falls_back_when_torrent_snapshot_fails() -> Result<()> {
        let temp = temp_dir()?;
        let library_root = temp.path().join("library");
        std::fs::create_dir_all(&library_root)?;

        let mut snapshot = sample_snapshot(AppMode::Active);
        snapshot.fs_policy.library_root = library_root.to_string_lossy().into_owned();
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig {
            snapshot,
            fail: false,
        });
        let state = state_with_handles(
            config,
            StubInspector {
                statuses: Vec::new(),
                fail_list: true,
            },
        )?;

        let Json(body) = dashboard(State(Arc::clone(&state))).await?;
        assert_eq!(body.download_bps, 0);
        assert_eq!(body.upload_bps, 0);
        assert_eq!(body.active, 0);
        assert_eq!(body.paused, 0);
        assert_eq!(body.completed, 0);
        assert!(
            state
                .current_health_degraded()
                .iter()
                .any(|component| component == "dashboard_torrents")
        );
        Ok(())
    }
}
