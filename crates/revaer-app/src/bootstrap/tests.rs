use super::*;
use revaer_config::{SettingsChangeset, SettingsFacade};
use revaer_events::Event;
use revaer_test_support::postgres::start_postgres;
use std::ffi::OsString;
use std::net::TcpListener;
use std::os::unix::ffi::OsStringExt;
use tokio::runtime::Runtime;
use tokio::time::timeout;
use tokio_stream::StreamExt;

async fn get_app_profile_with_retry(
    config: &ConfigService,
) -> AppResult<revaer_config::AppProfile> {
    let mut attempts_remaining = 10_u8;
    loop {
        match config.get_app_profile().await {
            Ok(profile) => return Ok(profile),
            Err(_) if attempts_remaining > 1 => {
                attempts_remaining -= 1;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(err) => return Err(AppError::config("config_service.get_app_profile", err)),
        }
    }
}

#[test]
fn secret_session_from_values_validate_presence_and_bounds() {
    assert!(matches!(secret_session_from_values(None, None), Ok(None)));
    assert!(matches!(
        secret_session_from_values(Some(" key-id "), Some(" secret-value ")),
        Ok(Some(_))
    ));

    let missing_secret = secret_session_from_values(Some("key-id"), None)
        .expect_err("missing secret env should fail");
    assert!(matches!(
        missing_secret,
        AppError::MissingEnv {
            name: "REVAER_SECRET_KEY"
        }
    ));

    let missing_key_id = secret_session_from_values(None, Some("secret-value"))
        .expect_err("missing key id env should fail");
    assert!(matches!(
        missing_key_id,
        AppError::MissingEnv {
            name: "REVAER_SECRET_KEY_ID"
        }
    ));

    let empty_key_id = secret_session_from_values(Some("   "), Some("secret-value"))
        .expect_err("blank key id should fail");
    assert!(matches!(
        empty_key_id,
        AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY_ID",
            reason: "empty",
            value: None,
        }
    ));

    let too_long_key_id = "k".repeat(DbSessionConfig::SECRET_KEY_ID_MAX_LEN + 1);
    let too_long_error = secret_session_from_values(Some(&too_long_key_id), Some("secret-value"))
        .expect_err("oversized key id should fail");
    assert!(matches!(
        too_long_error,
        AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY_ID",
            reason: "too_long",
            value: None,
        }
    ));
}

#[test]
fn optional_env_var_with_rejects_non_unicode_values() {
    let error = optional_env_var_with("REVAER_SECRET_KEY", |_| {
        Err(std::env::VarError::NotUnicode(OsString::from_vec(vec![
            0xff,
        ])))
    })
    .expect_err("non-unicode env vars should be rejected");

    assert!(matches!(
        error,
        AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY",
            reason: "env_not_unicode",
            value: None,
        }
    ));
}

#[test]
fn otel_and_guardrail_helpers_cover_expected_modes() -> AppResult<()> {
    assert!(env_flag_value(Some("true")));
    assert!(env_flag_value(Some(" On ")));
    assert!(!env_flag_value(Some("off")));
    assert!(!env_flag_value(None));

    let disabled = otel_config_from_values(
        false,
        "revaer-app".to_string(),
        Some("http://collector:4317".to_string()),
    );
    assert!(disabled.is_none());

    let enabled = otel_config_from_values(
        true,
        "revaer-app".to_string(),
        Some("http://collector:4317".to_string()),
    )
    .expect("enabled otel config should be returned");
    assert!(enabled.enabled);
    assert_eq!(enabled.service_name.as_ref(), "revaer-app");
    assert_eq!(enabled.endpoint.as_deref(), Some("http://collector:4317"));

    let telemetry = Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
    let events = EventBus::with_capacity(4);
    let err = enforce_loopback_guard(
        &AppMode::Setup,
        IpAddr::from([10, 0, 0, 1]),
        &telemetry,
        &events,
    )
    .expect_err("setup mode must reject non-loopback bind addresses");
    assert!(matches!(
        err,
        AppError::InvalidConfig {
            field: "bind_addr",
            reason: "non_loopback_in_setup",
            value: Some(_),
        }
    ));
    let emitted = events.backlog_since(0);
    assert_eq!(emitted.len(), 1);
    assert!(matches!(
        &emitted[0].event,
        Event::HealthChanged { degraded } if degraded == &vec!["loopback_guard".to_string()]
    ));
    let rendered = telemetry
        .render()
        .map_err(|err| AppError::telemetry("telemetry.render", err))?;
    assert!(rendered.contains("config_guardrail_violations_total"));

    enforce_loopback_guard(
        &AppMode::Setup,
        IpAddr::from([127, 0, 0, 1]),
        &telemetry,
        &events,
    )?;
    enforce_loopback_guard(
        &AppMode::Active,
        IpAddr::from([10, 0, 0, 1]),
        &telemetry,
        &events,
    )?;
    Ok(())
}

#[tokio::test]
async fn bootstrap_dependencies_from_database_url_track_persisted_settings_changes() -> AppResult<()>
{
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping bootstrap_dependencies_from_database_url_track_persisted_settings_changes: {err}"
            );
            return Ok(());
        }
    };

    let BootstrapDependencies {
        config,
        snapshot,
        mut watcher,
        events,
        telemetry,
        ..
    } = BootstrapDependencies::from_database_url(postgres.connection_string().to_string()).await?;
    watcher.disable_listen();

    assert!(
        snapshot.revision > 0,
        "expected migrated configuration revision"
    );
    assert!(
        events.last_event_id().is_none(),
        "bootstrap should not emit startup events"
    );
    let rendered = telemetry
        .render()
        .map_err(|err| AppError::telemetry("telemetry.render", err))?;
    assert!(rendered.contains("config_guardrail_violations_total"));

    let mut app_profile = get_app_profile_with_retry(&config).await?;
    app_profile.immutable_keys.clear();
    app_profile.instance_name = "Bootstrap watcher".to_string();
    let applied = config
        .apply_changeset(
            "tester",
            "bootstrap-deps-watch",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await
        .map_err(|err| AppError::config("config_service.apply_changeset", err))?;

    let updated = timeout(Duration::from_secs(8), watcher.next())
        .await
        .map_err(|_| AppError::MissingState {
            field: "config_watcher_update",
            value: None,
        })?
        .map_err(|err| AppError::config("config_watcher.next", err))?;
    assert!(updated.revision >= applied.revision);
    assert_eq!(updated.app_profile.instance_name, "Bootstrap watcher");
    Ok(())
}

#[tokio::test]
async fn build_api_server_accepts_bootstrapped_config() -> AppResult<()> {
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!("skipping build_api_server_accepts_bootstrapped_config: {err}");
            return Ok(());
        }
    };

    let config = ConfigService::new(postgres.connection_string().to_string())
        .await
        .map_err(|err| AppError::config("config_service.new", err))?;
    let events = EventBus::with_capacity(4);
    let telemetry = Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;

    let server = build_api_server(&config, &events, None, telemetry)?;
    drop(server);
    Ok(())
}

#[tokio::test]
async fn run_bootstrap_services_rejects_public_setup_bind_from_dependencies() -> AppResult<()> {
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_bootstrap_services_rejects_public_setup_bind_from_dependencies: {err}"
            );
            return Ok(());
        }
    };

    let mut dependencies =
        BootstrapDependencies::from_database_url(postgres.connection_string().to_string()).await?;
    dependencies.snapshot.app_profile.mode = AppMode::Setup;
    dependencies.snapshot.app_profile.bind_addr = IpAddr::from([10, 0, 0, 1]);

    let err = run_bootstrap_services(dependencies)
        .await
        .expect_err("public setup bind should fail before starting services");
    assert!(matches!(
        err,
        AppError::InvalidConfig {
            field: "bind_addr",
            reason: "non_loopback_in_setup",
            value: Some(_),
        }
    ));
    Ok(())
}

#[tokio::test]
async fn run_bootstrap_services_rejects_zero_http_port_from_dependencies() -> AppResult<()> {
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_bootstrap_services_rejects_zero_http_port_from_dependencies: {err}"
            );
            return Ok(());
        }
    };

    let mut dependencies =
        BootstrapDependencies::from_database_url(postgres.connection_string().to_string()).await?;
    dependencies.snapshot.app_profile.http_port = 0;

    let err = run_bootstrap_services(dependencies)
        .await
        .expect_err("zero http port should fail before starting services");
    assert!(matches!(
        err,
        AppError::InvalidConfig {
            field: "http_port",
            reason: "zero",
            value: Some(_),
        }
    ));
    Ok(())
}

#[tokio::test]
async fn run_bootstrap_services_surfaces_bind_failures_for_valid_snapshot() -> AppResult<()> {
    let postgres = match start_postgres() {
        Ok(database) => database,
        Err(err) => {
            eprintln!(
                "skipping run_bootstrap_services_surfaces_bind_failures_for_valid_snapshot: {err}"
            );
            return Ok(());
        }
    };

    let reserved_listener =
        TcpListener::bind((IpAddr::from([127, 0, 0, 1]), 0)).map_err(|source| AppError::Io {
            operation: "tcp_listener.bind",
            path: None,
            source,
        })?;
    let reserved_port = i32::from(
        reserved_listener
            .local_addr()
            .map_err(|source| AppError::Io {
                operation: "tcp_listener.local_addr",
                path: None,
                source,
            })?
            .port(),
    );

    let mut dependencies =
        BootstrapDependencies::from_database_url(postgres.connection_string().to_string()).await?;
    dependencies.snapshot.app_profile.mode = AppMode::Setup;
    dependencies.snapshot.app_profile.bind_addr = IpAddr::from([127, 0, 0, 1]);
    dependencies.snapshot.app_profile.http_port = reserved_port;

    let err = run_bootstrap_services(dependencies)
        .await
        .expect_err("occupied listener should fail api server startup");
    assert!(matches!(err, AppError::ApiServer { .. }), "{err:?}");
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
    use tokio_stream::StreamExt;
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

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> TorrentResult<()> {
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
            seed_choking_algorithm: revaer_config::EngineProfile::default_seed_choking_algorithm(),
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

#[test]
fn secret_session_none_returns_none() {
    let result = secret_session_from_values(None, None);

    assert!(matches!(result, Ok(None)));
}

#[test]
fn secret_session_both_present_returns_some() {
    let result = secret_session_from_values(Some("key-id"), Some("secret-value"));

    assert!(matches!(result, Ok(Some(_))));
}

#[test]
fn secret_session_missing_key_errors() {
    let result = secret_session_from_values(None, Some("secret-value"));

    assert!(matches!(
        result,
        Err(AppError::MissingEnv {
            name: "REVAER_SECRET_KEY_ID"
        })
    ));
}

#[test]
fn secret_session_missing_secret_errors() {
    let result = secret_session_from_values(Some("key-id"), None);

    assert!(matches!(
        result,
        Err(AppError::MissingEnv {
            name: "REVAER_SECRET_KEY"
        })
    ));
}

#[test]
fn secret_session_empty_key_errors() {
    let result = secret_session_from_values(Some("   "), Some("secret-value"));

    assert!(matches!(
        result,
        Err(AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY_ID",
            reason: "empty",
            value: None
        })
    ));
}

#[test]
fn secret_session_empty_secret_errors() {
    let result = secret_session_from_values(Some("key-id"), Some(" "));

    assert!(matches!(
        result,
        Err(AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY",
            reason: "empty",
            value: None
        })
    ));
}

#[test]
fn secret_session_key_too_long_errors() {
    let too_long = "k".repeat(DbSessionConfig::SECRET_KEY_ID_MAX_LEN + 1);
    let result = secret_session_from_values(Some(&too_long), Some("secret-value"));

    assert!(matches!(
        result,
        Err(AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY_ID",
            reason: "too_long",
            value: None
        })
    ));
}

#[test]
fn secret_session_secret_too_long_errors() {
    let too_long = "s".repeat(DbSessionConfig::SECRET_KEY_MAX_LEN + 1);
    let result = secret_session_from_values(Some("key-id"), Some(&too_long));

    assert!(matches!(
        result,
        Err(AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY",
            reason: "too_long",
            value: None
        })
    ));
}

#[test]
fn optional_env_var_rejects_non_unicode_values() {
    let result = optional_env_var_with("REVAER_SECRET_KEY_ID", |_| {
        Err(std::env::VarError::NotUnicode(std::ffi::OsString::from(
            "invalid",
        )))
    });

    assert!(matches!(
        result,
        Err(AppError::InvalidConfig {
            field: "REVAER_SECRET_KEY_ID",
            reason: "env_not_unicode",
            value: None,
        })
    ));
}

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
fn bootstrap_http_port_validates_zero_and_out_of_range_values() {
    assert_eq!(
        bootstrap_http_port(7070).expect("valid port should pass"),
        7070
    );

    let zero_error = bootstrap_http_port(0).expect_err("zero port should fail");
    assert!(matches!(
        zero_error,
        AppError::InvalidConfig {
            field: "http_port",
            reason: "zero",
            value: Some(_),
        }
    ));

    let range_error =
        bootstrap_http_port(i32::from(u16::MAX) + 1).expect_err("out of range port should fail");
    assert!(matches!(
        range_error,
        AppError::InvalidConfig {
            field: "http_port",
            reason: "out_of_range",
            value: Some(_),
        }
    ));
}

#[test]
fn otel_config_from_values_preserves_missing_endpoint() -> AppResult<()> {
    let cfg = otel_config_from_values(true, "svc".into(), None).ok_or_else(|| {
        AppError::MissingState {
            field: "otel_config",
            value: None,
        }
    })?;
    assert_eq!(cfg.service_name.as_ref(), "svc");
    assert!(cfg.endpoint.is_none());
    Ok(())
}

#[test]
fn indexer_runtime_module_stays_dependency_injected() {
    let source = include_str!("../indexers.rs");
    let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);

    for forbidden in [
        "std::env::",
        "Metrics::new(",
        "ConfigService::new(",
        "EventBus::new(",
        "RuntimeStore::new(",
        "ApiServer::new(",
    ] {
        assert!(
            !production_source.contains(forbidden),
            "indexer runtime module must not construct or read '{forbidden}' directly"
        );
    }

    assert!(
        production_source.contains(
            "pub(crate) const fn new(config: Arc<ConfigService>, telemetry: Metrics) -> Self"
        ),
        "indexer service must receive infrastructure collaborators from bootstrap"
    );
}

#[test]
fn bootstrap_owns_indexer_wiring_and_environment_reads() {
    let source = include_str!("../bootstrap.rs");

    for required in [
        "std::env::var(\"DATABASE_URL\")",
        "optional_env_var(\"REVAER_SECRET_KEY_ID\")",
        "optional_env_var(\"REVAER_SECRET_KEY\")",
        "Metrics::new()",
        "EventBus::new()",
        "RuntimeStore::new(",
        "IndexerService::new(",
        "IndexerRuntime::new(",
    ] {
        assert!(
            source.contains(required),
            "bootstrap must remain the wiring boundary for '{required}'"
        );
    }
}

#[test]
fn loopback_guard_allows_loopback_address() -> AppResult<()> {
    let events = EventBus::with_capacity(4);
    let metrics = Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
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
    let metrics = Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;
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
