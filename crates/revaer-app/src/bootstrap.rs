use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr};
#[cfg(feature = "libtorrent")]
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "libtorrent")]
use std::time::Instant;

use crate::error::{AppError, AppResult};
use revaer_api::TorrentHandles;
use revaer_config::{AppMode, ConfigService, ConfigSnapshot};
use revaer_events::EventBus;
use revaer_telemetry::{GlobalContextGuard, LoggingConfig, Metrics, OpenTelemetryConfig};
#[cfg(feature = "libtorrent")]
use tracing::warn;
use tracing::{error, info};

use revaer_runtime::RuntimeStore;

#[cfg(feature = "libtorrent")]
use crate::orchestrator::{
    EngineConfigurator, LibtorrentOrchestratorDeps, spawn_libtorrent_orchestrator,
};
#[cfg(feature = "libtorrent")]
use revaer_torrent_core::{TorrentEngine, TorrentInspector, TorrentWorkflow};

/// Dependencies required to bootstrap the Revaer application.
pub(crate) struct BootstrapDependencies {
    logging: LoggingConfig<'static>,
    otel_config: Option<OpenTelemetryConfig<'static>>,
    config: ConfigService,
    snapshot: ConfigSnapshot,
    watcher: revaer_config::ConfigWatcher,
    events: EventBus,
    telemetry: Metrics,
    #[cfg(feature = "libtorrent")]
    libtorrent: Option<LibtorrentOrchestratorDeps>,
}

impl BootstrapDependencies {
    /// Construct production dependencies from the environment for the binary entrypoint.
    pub(crate) async fn from_env() -> AppResult<Self> {
        let database_url = database_url_from_env()?;
        Self::from_database_url(database_url).await
    }

    pub(crate) async fn from_database_url(database_url: String) -> AppResult<Self> {
        let logging = LoggingConfig::default();
        let otel_config = load_otel_config_from_env();

        let config = ConfigService::new(database_url)
            .await
            .map_err(|err| AppError::config("config_service.new", err))?;

        let (snapshot, watcher) = config
            .watch_settings(Duration::from_secs(5))
            .await
            .map_err(|err| AppError::config("config_service.watch_settings", err))?;

        let events = EventBus::new();
        let telemetry =
            Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;

        #[cfg(feature = "libtorrent")]
        let runtime = Some(
            RuntimeStore::new(config.pool().clone())
                .await
                .map_err(|err| AppError::runtime("runtime_store.new", err))?,
        );
        #[cfg(not(feature = "libtorrent"))]
        let _runtime: Option<RuntimeStore> = None;

        #[cfg(feature = "libtorrent")]
        let libtorrent = Some(LibtorrentOrchestratorDeps::new(
            &events, &telemetry, runtime,
        )?);

        Ok(Self {
            logging,
            otel_config,
            config,
            snapshot,
            watcher,
            events,
            telemetry,
            #[cfg(feature = "libtorrent")]
            libtorrent,
        })
    }
}

fn database_url_from_env() -> AppResult<String> {
    std::env::var("DATABASE_URL").map_err(|_| AppError::MissingEnv {
        name: "DATABASE_URL",
    })
}

/// Entry point for the Revaer application boot sequence.
///
/// # Errors
///
/// Returns an error if dependency construction or application startup fails.
pub async fn run_app() -> AppResult<()> {
    let dependencies = BootstrapDependencies::from_env().await?;
    Box::pin(run_app_with(dependencies)).await
}

/// Boot sequence using a provided database URL.
///
/// # Errors
///
/// Returns an error if dependency construction or application startup fails.
pub async fn run_app_with_database_url(database_url: String) -> AppResult<()> {
    let dependencies = BootstrapDependencies::from_database_url(database_url).await?;
    Box::pin(run_app_with(dependencies)).await
}

/// Boot sequence that relies entirely on injected dependencies to simplify testing.
pub(crate) async fn run_app_with(dependencies: BootstrapDependencies) -> AppResult<()> {
    let otel_ref = dependencies
        .otel_config
        .as_ref()
        .map(|cfg| cfg as &OpenTelemetryConfig);
    let _otel_guard = revaer_telemetry::init_logging_with_otel(&dependencies.logging, otel_ref)
        .map_err(|err| AppError::telemetry("telemetry.init", err))?;
    let _context = GlobalContextGuard::new("bootstrap");

    info!("Revaer application bootstrap starting");

    let BootstrapDependencies {
        logging: _,
        otel_config: _,
        config,
        snapshot,
        watcher,
        events,
        telemetry,
        #[cfg(feature = "libtorrent")]
        libtorrent,
    } = dependencies;

    #[cfg(feature = "libtorrent")]
    let (fsops_worker, config_task, torrent_handles) = {
        let libtorrent = libtorrent.ok_or(AppError::MissingDependency { name: "libtorrent" })?;
        let (_engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &events,
            snapshot.fs_policy.clone(),
            snapshot.engine_profile.clone(),
            libtorrent,
            Some(Arc::new(config.clone())),
        )
        .await?;
        info!("Filesystem post-processing orchestrator ready");
        let workflow: Arc<dyn TorrentWorkflow> = orchestrator.clone();
        let inspector: Arc<dyn TorrentInspector> = orchestrator.clone();
        let handles = TorrentHandles::new(workflow, inspector);
        let config_task = spawn_config_watch_task(
            watcher,
            Arc::clone(&orchestrator),
            events.clone(),
            telemetry.clone(),
        );
        (worker, config_task, Some(handles))
    };

    #[cfg(not(feature = "libtorrent"))]
    let torrent_handles: Option<TorrentHandles> = {
        let _ = watcher;
        let _ = &snapshot.fs_policy;
        let _ = &snapshot.engine_profile;
        None
    };

    let api = revaer_api::ApiServer::new(
        config.clone(),
        events.clone(),
        torrent_handles,
        telemetry.clone(),
    )
    .map_err(|err| AppError::api_server("api_server.new", err))?;

    enforce_loopback_guard(
        &snapshot.app_profile.mode,
        snapshot.app_profile.bind_addr,
        &telemetry,
        &events,
    )?;

    let port =
        u16::try_from(snapshot.app_profile.http_port).map_err(|_| AppError::InvalidConfig {
            field: "http_port",
            reason: "out_of_range",
            value: Some(snapshot.app_profile.http_port.to_string()),
        })?;
    if port == 0 {
        return Err(AppError::InvalidConfig {
            field: "http_port",
            reason: "zero",
            value: Some(snapshot.app_profile.http_port.to_string()),
        });
    }

    let addr = SocketAddr::new(snapshot.app_profile.bind_addr, port);
    info!(addr = %addr, "Launching API listener");

    let serve_result = api.serve(addr).await;

    #[cfg(feature = "libtorrent")]
    {
        if !fsops_worker.is_finished() {
            fsops_worker.abort();
        }
        if let Err(err) = fsops_worker.await {
            warn!(error = %err, "fsops worker join failed");
        }

        if !config_task.is_finished() {
            config_task.abort();
        }
        if let Err(err) = config_task.await {
            warn!(error = %err, "config watcher task join failed");
        }
    }

    serve_result.map_err(|err| AppError::api_server("api_server.serve", err))?;
    info!("API server shutdown complete");
    Ok(())
}

fn load_otel_config_from_env() -> Option<OpenTelemetryConfig<'static>> {
    let enabled = env_flag("REVAER_ENABLE_OTEL");
    let service_name =
        std::env::var("REVAER_OTEL_SERVICE_NAME").unwrap_or_else(|_| "revaer-app".to_string());
    let endpoint = std::env::var("REVAER_OTEL_EXPORTER").ok();
    otel_config_from_values(enabled, service_name, endpoint)
}

fn env_flag(name: &str) -> bool {
    env_flag_value(std::env::var(name).ok().as_deref())
}

fn env_flag_value(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn otel_config_from_values(
    enabled: bool,
    service_name: String,
    endpoint: Option<String>,
) -> Option<OpenTelemetryConfig<'static>> {
    if !enabled {
        return None;
    }
    Some(OpenTelemetryConfig {
        enabled: true,
        service_name: Cow::Owned(service_name),
        endpoint: endpoint.map(Cow::Owned),
    })
}

#[cfg(feature = "libtorrent")]
fn spawn_config_watch_task<E>(
    mut watcher: revaer_config::ConfigWatcher,
    orchestrator: Arc<crate::orchestrator::TorrentOrchestrator<E>>,
    events: EventBus,
    telemetry: Metrics,
) -> tokio::task::JoinHandle<()>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    tokio::spawn(async move {
        const APPLY_SLA: Duration = Duration::from_secs(2);
        let mut config_degraded = false;
        loop {
            let wait_started = Instant::now();
            match watcher.next().await {
                Ok(snapshot) => {
                    telemetry.observe_config_watch_latency(wait_started.elapsed());
                    apply_config_snapshot(
                        snapshot,
                        &orchestrator,
                        &events,
                        &telemetry,
                        &mut config_degraded,
                        APPLY_SLA,
                    )
                    .await;
                }
                Err(err) => {
                    telemetry.inc_config_update_failure();
                    warn!(error = %err, "configuration watcher terminated");
                    set_config_degraded(&events, &mut config_degraded, true);
                    break;
                }
            }
        }
    })
}

#[cfg(feature = "libtorrent")]
async fn apply_config_snapshot<E>(
    snapshot: revaer_config::ConfigSnapshot,
    orchestrator: &crate::orchestrator::TorrentOrchestrator<E>,
    events: &EventBus,
    telemetry: &Metrics,
    config_degraded: &mut bool,
    apply_sla: Duration,
) where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    orchestrator
        .update_fs_policy(snapshot.fs_policy.clone())
        .await;
    let apply_started = Instant::now();
    match orchestrator
        .update_engine_profile(snapshot.engine_profile.clone())
        .await
    {
        Ok(()) => {
            let apply_elapsed = apply_started.elapsed();
            telemetry.observe_config_apply_latency(apply_elapsed);
            let mut description = format!(
                "watcher revision {} applied in {}ms",
                snapshot.revision,
                apply_elapsed.as_millis()
            );
            if apply_elapsed > apply_sla {
                telemetry.inc_config_watch_slow();
                warn!(
                    revision = snapshot.revision,
                    elapsed_ms = apply_elapsed.as_millis(),
                    "configuration update exceeded latency guard rail"
                );
                description = format!(
                    "watcher revision {} applied after {}ms (exceeded guard rail)",
                    snapshot.revision,
                    apply_elapsed.as_millis()
                );
                set_config_degraded(events, config_degraded, true);
            } else {
                set_config_degraded(events, config_degraded, false);
            }
            publish_event(
                events,
                revaer_events::Event::SettingsChanged { description },
            );
            info!(
                revision = snapshot.revision,
                elapsed_ms = apply_elapsed.as_millis(),
                "applied configuration update from watcher"
            );
        }
        Err(err) => {
            telemetry.inc_config_update_failure();
            warn!(
                error = %err,
                revision = snapshot.revision,
                "failed to apply engine profile update from watcher"
            );
            let description = format!(
                "failed to apply watcher revision {}: {}",
                snapshot.revision, err
            );
            publish_event(
                events,
                revaer_events::Event::SettingsChanged { description },
            );
            set_config_degraded(events, config_degraded, true);
        }
    }
}

#[cfg(feature = "libtorrent")]
fn set_config_degraded(events: &EventBus, config_degraded: &mut bool, degraded: bool) {
    if *config_degraded == degraded {
        return;
    }
    let degraded_list = if degraded {
        vec!["config_watcher".to_string()]
    } else {
        Vec::new()
    };
    publish_event(
        events,
        revaer_events::Event::HealthChanged {
            degraded: degraded_list,
        },
    );
    *config_degraded = degraded;
}

fn enforce_loopback_guard(
    mode: &AppMode,
    bind_addr: IpAddr,
    telemetry: &Metrics,
    events: &EventBus,
) -> AppResult<()> {
    if matches!(mode, AppMode::Setup) && !bind_addr.is_loopback() {
        error!(
            bind_addr = %bind_addr,
            "refusing to bind setup mode API listener to non-loopback address"
        );
        telemetry.inc_guardrail_violation();
        publish_event(
            events,
            revaer_events::Event::HealthChanged {
                degraded: vec!["loopback_guard".to_string()],
            },
        );
        return Err(AppError::InvalidConfig {
            field: "bind_addr",
            reason: "non_loopback_in_setup",
            value: Some(bind_addr.to_string()),
        });
    }
    Ok(())
}

fn publish_event(events: &EventBus, event: revaer_events::Event) {
    if let Err(error) = events.publish(event) {
        tracing::warn!(
            event_id = error.event_id(),
            event_kind = error.event_kind(),
            error = %error,
            "failed to publish event"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use tokio_stream::StreamExt;

    #[test]
    fn env_flag_handles_truthy_and_falsey() {
        assert!(env_flag_value(Some("TrUe")));
        assert!(!env_flag_value(Some("no")));
        assert!(!env_flag_value(None));
    }

    #[test]
    fn load_otel_config_reads_values() -> AppResult<()> {
        let cfg = otel_config_from_values(true, "svc".into(), Some("http://collector".into()))
            .ok_or_else(|| AppError::MissingState {
                field: "otel_config",
                value: None,
            })?;
        assert_eq!(cfg.service_name.as_ref(), "svc");
        assert_eq!(cfg.endpoint.as_deref(), Some("http://collector"));
        assert!(otel_config_from_values(false, "svc".into(), None).is_none());
        Ok(())
    }

    #[test]
    fn loopback_guard_allows_loopback_address() -> AppResult<()> {
        let events = EventBus::with_capacity(4);
        let metrics =
            Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
        enforce_loopback_guard(
            &AppMode::Setup,
            IpAddr::from([127, 0, 0, 1]),
            &metrics,
            &events,
        )?;

        enforce_loopback_guard(
            &AppMode::Active,
            IpAddr::from([192, 168, 1, 1]),
            &metrics,
            &events,
        )?;
        Ok(())
    }

    #[test]
    fn loopback_guard_rejects_public_interface_during_setup() -> AppResult<()> {
        let events = EventBus::with_capacity(4);
        let metrics =
            Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
        let mut stream = events.subscribe(None);
        let runtime = Runtime::new().map_err(|err| AppError::Io {
            operation: "runtime.new",
            path: None,
            source: err,
        })?;

        let result = enforce_loopback_guard(
            &AppMode::Setup,
            IpAddr::from([192, 168, 10, 20]),
            &metrics,
            &events,
        );
        assert!(result.is_err(), "expected guard rail to reject address");

        let envelope = runtime
            .block_on(async { stream.next().await })
            .ok_or_else(|| AppError::MissingState {
                field: "health_event",
                value: None,
            })?
            .map_err(|err| AppError::InvalidConfig {
                field: "event_stream",
                reason: "recv_error",
                value: Some(err.to_string()),
            })?;
        assert!(matches!(
            envelope.event,
            revaer_events::Event::HealthChanged { .. }
        ));
        Ok(())
    }

    #[cfg(feature = "libtorrent")]
    mod libtorrent_tests {
        use super::*;
        use crate::engine_config::EngineRuntimePlan;
        use async_trait::async_trait;
        use revaer_config::engine_profile::{
            AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig,
        };
        use revaer_config::{AppAuthMode, AppProfile, ConfigSnapshot, FsPolicy, TelemetryConfig};
        use revaer_fsops::FsOpsService;
        use revaer_torrent_core::{
            AddTorrent, RemoveTorrent, TorrentError, TorrentRateLimit, TorrentResult,
        };
        use tokio::time::{Duration, timeout};
        use uuid::Uuid;

        #[derive(Debug)]
        struct TestEngine {
            fail_apply: bool,
            fail_update: bool,
            delay: Option<Duration>,
        }

        #[async_trait]
        impl TorrentEngine for TestEngine {
            async fn add_torrent(&self, _request: AddTorrent) -> TorrentResult<()> {
                Ok(())
            }

            async fn remove_torrent(
                &self,
                _id: Uuid,
                _options: RemoveTorrent,
            ) -> TorrentResult<()> {
                Ok(())
            }

            async fn update_limits(
                &self,
                _id: Option<Uuid>,
                _limits: TorrentRateLimit,
            ) -> TorrentResult<()> {
                if self.fail_update {
                    return Err(TorrentError::Unsupported {
                        operation: "update_limits",
                    });
                }
                Ok(())
            }
        }

        #[async_trait]
        impl EngineConfigurator for TestEngine {
            async fn apply_engine_plan(&self, _plan: &EngineRuntimePlan) -> TorrentResult<()> {
                if let Some(delay) = self.delay {
                    tokio::time::sleep(delay).await;
                }
                if self.fail_apply {
                    return Err(TorrentError::Unsupported {
                        operation: "apply_engine_plan",
                    });
                }
                Ok(())
            }
        }

        fn sample_app_profile(mode: AppMode) -> AppProfile {
            AppProfile {
                id: Uuid::new_v4(),
                instance_name: "bootstrap-test".to_string(),
                mode,
                auth_mode: AppAuthMode::NoAuth,
                version: 1,
                http_port: 7070,
                bind_addr: IpAddr::from([127, 0, 0, 1]),
                local_networks: vec!["127.0.0.0/8".to_string()],
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            }
        }

        fn sample_engine_profile() -> revaer_config::EngineProfile {
            revaer_config::EngineProfile {
                id: Uuid::new_v4(),
                implementation: "libtorrent".to_string(),
                listen_port: Some(6_881),
                listen_interfaces: Vec::new(),
                ipv6_mode: "disabled".to_string(),
                anonymous_mode: false.into(),
                force_proxy: false.into(),
                prefer_rc4: false.into(),
                allow_multiple_connections_per_ip: false.into(),
                enable_outgoing_utp: false.into(),
                enable_incoming_utp: false.into(),
                dht: true,
                encryption: "prefer".to_string(),
                max_active: Some(4),
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
                choking_algorithm: revaer_config::EngineProfile::default_choking_algorithm(),
                seed_choking_algorithm:
                    revaer_config::EngineProfile::default_seed_choking_algorithm(),
                strict_super_seeding: false.into(),
                optimistic_unchoke_slots: None,
                max_queued_disk_bytes: None,
                resume_dir: ".server_root/resume".to_string(),
                download_root: ".server_root/downloads".to_string(),
                storage_mode: revaer_config::EngineProfile::default_storage_mode(),
                use_partfile: revaer_config::EngineProfile::default_use_partfile(),
                disk_read_mode: None,
                disk_write_mode: None,
                verify_piece_hashes: revaer_config::EngineProfile::default_verify_piece_hashes(),
                cache_size: None,
                cache_expiry: None,
                coalesce_reads: revaer_config::EngineProfile::default_coalesce_reads(),
                coalesce_writes: revaer_config::EngineProfile::default_coalesce_writes(),
                use_disk_cache_pool: revaer_config::EngineProfile::default_use_disk_cache_pool(),
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
            }
        }

        fn sample_fs_policy() -> FsPolicy {
            FsPolicy {
                id: Uuid::new_v4(),
                library_root: ".server_root/library".to_string(),
                extract: false,
                par2: "disabled".to_string(),
                flatten: false,
                move_mode: "copy".to_string(),
                cleanup_keep: Vec::new(),
                cleanup_drop: Vec::new(),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: Vec::new(),
            }
        }

        fn sample_snapshot() -> ConfigSnapshot {
            let engine_profile = sample_engine_profile();
            ConfigSnapshot {
                revision: 3,
                app_profile: sample_app_profile(AppMode::Active),
                engine_profile_effective: revaer_config::normalize_engine_profile(&engine_profile),
                engine_profile,
                fs_policy: sample_fs_policy(),
            }
        }

        #[tokio::test]
        async fn apply_config_snapshot_marks_degraded_on_slow_apply() -> AppResult<()> {
            let events = EventBus::with_capacity(8);
            let metrics =
                Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
            let fsops = FsOpsService::new(events.clone(), metrics.clone());
            let engine = Arc::new(TestEngine {
                fail_apply: false,
                fail_update: false,
                delay: Some(Duration::from_millis(5)),
            });
            let orchestrator = Arc::new(crate::orchestrator::TorrentOrchestrator::new(
                engine,
                fsops,
                events.clone(),
                sample_fs_policy(),
                sample_engine_profile(),
                None,
                None,
            ));
            let mut stream = events.subscribe(None);
            let mut degraded = false;

            apply_config_snapshot(
                sample_snapshot(),
                &orchestrator,
                &events,
                &metrics,
                &mut degraded,
                Duration::from_millis(1),
            )
            .await;

            assert!(degraded, "expected config watcher to mark degraded");

            let mut saw_settings = false;
            let mut saw_health = false;
            timeout(Duration::from_secs(2), async {
                for _ in 0..2 {
                    if let Some(Ok(envelope)) = stream.next().await {
                        match envelope.event {
                            revaer_events::Event::SettingsChanged { .. } => saw_settings = true,
                            revaer_events::Event::HealthChanged { degraded } => {
                                saw_health = !degraded.is_empty();
                            }
                            _ => {}
                        }
                    }
                }
            })
            .await
            .map_err(|_| AppError::MissingState {
                field: "config_events",
                value: None,
            })?;

            assert!(saw_settings, "expected settings change event");
            assert!(saw_health, "expected health degraded event");
            Ok(())
        }

        #[tokio::test]
        async fn apply_config_snapshot_marks_degraded_on_failure() -> AppResult<()> {
            let events = EventBus::with_capacity(8);
            let metrics =
                Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
            let fsops = FsOpsService::new(events.clone(), metrics.clone());
            let engine = Arc::new(TestEngine {
                fail_apply: true,
                fail_update: false,
                delay: None,
            });
            let orchestrator = Arc::new(crate::orchestrator::TorrentOrchestrator::new(
                engine,
                fsops,
                events.clone(),
                sample_fs_policy(),
                sample_engine_profile(),
                None,
                None,
            ));
            let mut stream = events.subscribe(None);
            let mut degraded = false;

            apply_config_snapshot(
                sample_snapshot(),
                &orchestrator,
                &events,
                &metrics,
                &mut degraded,
                Duration::from_secs(2),
            )
            .await;

            assert!(degraded, "expected degraded mode after failure");

            let mut saw_settings = false;
            let mut saw_health = false;
            timeout(Duration::from_secs(2), async {
                for _ in 0..2 {
                    if let Some(Ok(envelope)) = stream.next().await {
                        match envelope.event {
                            revaer_events::Event::SettingsChanged { .. } => saw_settings = true,
                            revaer_events::Event::HealthChanged { degraded } => {
                                saw_health = !degraded.is_empty();
                            }
                            _ => {}
                        }
                    }
                }
            })
            .await
            .map_err(|_| AppError::MissingState {
                field: "config_events",
                value: None,
            })?;

            assert!(saw_settings, "expected settings change event");
            assert!(saw_health, "expected health degraded event");
            Ok(())
        }
    }
}
