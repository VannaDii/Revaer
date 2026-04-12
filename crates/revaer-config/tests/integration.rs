use chrono::Weekday;
use revaer_config::{
    ApiKeyPatch, ApiKeyRateLimit, AppMode, ConfigError, ConfigService, DbSessionConfig,
    SecretPatch, SettingsChangeset, SettingsFacade,
    engine_profile::{AltSpeedConfig, AltSpeedSchedule, MAX_RATE_LIMIT_BPS},
};
use revaer_config::{
    AppAuthMode, LabelKind, LabelPolicy, SettingsPayload, TelemetryConfig,
    engine_profile::{
        IpFilterConfig, PeerClassConfig, PeerClassesConfig, TrackerAuthConfig, TrackerConfig,
        TrackerProxyConfig, TrackerProxyType,
    },
    model::Toggle,
};
use revaer_data::config as data_config;
use revaer_test_support::postgres::start_postgres;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn config_service_applies_changes_and_tokens() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_applies_changes_and_tokens: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let (snapshot, mut stream) = service.watch_settings(Duration::from_millis(50)).await?;
    let mut app_profile = snapshot.app_profile.clone();
    app_profile.mode = AppMode::Active;
    app_profile.immutable_keys.clear();

    let mut engine_profile = snapshot.engine_profile.clone();
    engine_profile.dht = !snapshot.engine_profile.dht;
    engine_profile.listen_port = snapshot.engine_profile.listen_port;
    engine_profile.sequential_default = snapshot.engine_profile.sequential_default;
    engine_profile.resume_dir = resume_dir;
    engine_profile.download_root = download_root;

    let mut fs_policy = snapshot.fs_policy.clone();
    fs_policy.library_root = library_root.clone();
    fs_policy.flatten = !snapshot.fs_policy.flatten;
    fs_policy.allow_paths = vec![library_root];
    fs_policy.cleanup_keep = vec!["**/*.mkv".to_string()];
    fs_policy.cleanup_drop = Vec::new();
    fs_policy.move_mode = "copy".to_string();

    let changes = SettingsChangeset {
        app_profile: Some(app_profile),
        engine_profile: Some(engine_profile),
        fs_policy: Some(fs_policy),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "ci-key".to_string(),
            label: Some("ci".to_string()),
            enabled: Some(true),
            expires_at: None,
            secret: Some("super-secret".to_string()),
            rate_limit: Some(Some(ApiKeyRateLimit {
                burst: 10,
                replenish_period: Duration::from_secs(60),
            })),
        }],
        secrets: vec![SecretPatch::Set {
            name: "webhook_token".to_string(),
            value: "topsecret".to_string(),
        }],
    };

    let applied = service
        .apply_changeset("tester", "integration", changes)
        .await?;
    assert!(applied.app_profile.is_some());
    assert!(applied.engine_profile.is_some());
    assert!(applied.fs_policy.is_some());

    let issued = service
        .issue_setup_token(Duration::from_secs(60), "tester")
        .await?;
    service.validate_setup_token(&issued.plaintext).await?;
    service.consume_setup_token(&issued.plaintext).await?;

    let refreshed = service.snapshot().await?;
    assert!(refreshed.revision >= snapshot.revision);
    assert_eq!(refreshed.app_profile.mode, AppMode::Active);
    let profile = service.get_app_profile().await?;
    assert_eq!(profile.mode, AppMode::Active);
    let engine = service.get_engine_profile().await?;
    assert_eq!(
        engine.implementation,
        snapshot.engine_profile.implementation
    );
    let fs_policy = service.get_fs_policy().await?;
    assert_eq!(fs_policy.flatten, !snapshot.fs_policy.flatten);
    let auth = service
        .authenticate_api_key("ci-key", "super-secret")
        .await?
        .ok_or_else(|| anyhow::anyhow!("api key did not authenticate"))?;
    assert_eq!(auth.key_id, "ci-key");

    let updated = tokio::time::timeout(Duration::from_secs(10), stream.next()).await??;
    assert!(updated.revision >= applied.revision);
    assert_eq!(updated.app_profile.mode, AppMode::Active);
    Ok(())
}

#[tokio::test]
async fn engine_profile_update_normalizes_alt_speed() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping engine_profile_update_normalizes_alt_speed: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    let profile = service.get_engine_profile().await?;
    let mut update = profile.clone();
    update.alt_speed = AltSpeedConfig {
        download_bps: Some(MAX_RATE_LIMIT_BPS + 1),
        upload_bps: Some(-10),
        schedule: Some(AltSpeedSchedule {
            days: vec![Weekday::Wed, Weekday::Mon, Weekday::Mon],
            start_minutes: 60,
            end_minutes: 150,
        }),
    };

    let changes = SettingsChangeset {
        app_profile: None,
        engine_profile: Some(update),
        fs_policy: None,
        api_keys: Vec::new(),
        secrets: Vec::new(),
    };

    service
        .apply_changeset("tester", "normalize-alt-speed", changes)
        .await?;

    let refreshed = service.get_engine_profile().await?;
    assert_eq!(
        refreshed.alt_speed,
        AltSpeedConfig {
            download_bps: Some(MAX_RATE_LIMIT_BPS),
            upload_bps: None,
            schedule: Some(AltSpeedSchedule {
                days: vec![Weekday::Mon, Weekday::Wed],
                start_minutes: 60,
                end_minutes: 150,
            }),
        }
    );
    Ok(())
}

#[tokio::test]
async fn config_service_manages_secret_and_api_key_lifecycle() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_manages_secret_and_api_key_lifecycle: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    assert!(!service.has_api_keys().await?);
    assert!(service.get_secret("webhook_token").await?.is_none());

    let create = SettingsChangeset {
        app_profile: None,
        engine_profile: None,
        fs_policy: None,
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "integration-key".to_string(),
            label: Some("integration".to_string()),
            enabled: Some(true),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
            secret: Some("super-secret".to_string()),
            rate_limit: Some(Some(ApiKeyRateLimit {
                burst: 5,
                replenish_period: Duration::from_secs(30),
            })),
        }],
        secrets: vec![SecretPatch::Set {
            name: "webhook_token".to_string(),
            value: "hook-secret".to_string(),
        }],
    };
    service
        .apply_changeset("tester", "create-auth", create)
        .await?;

    assert!(service.has_api_keys().await?);
    assert_eq!(
        service.get_secret("webhook_token").await?,
        Some("hook-secret".to_string())
    );
    assert!(
        service
            .authenticate_api_key("integration-key", "super-secret")
            .await?
            .is_some()
    );

    let delete = SettingsChangeset {
        app_profile: None,
        engine_profile: None,
        fs_policy: None,
        api_keys: vec![ApiKeyPatch::Delete {
            key_id: "integration-key".to_string(),
        }],
        secrets: vec![SecretPatch::Delete {
            name: "webhook_token".to_string(),
        }],
    };
    service
        .apply_changeset("tester", "delete-auth", delete)
        .await?;

    assert!(!service.has_api_keys().await?);
    assert!(service.get_secret("webhook_token").await?.is_none());
    assert!(
        service
            .authenticate_api_key("integration-key", "super-secret")
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn setup_token_lifecycle_covers_invalid_and_consumed_states() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping setup_token_lifecycle_covers_invalid_and_consumed_states: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    let invalid_ttl = service
        .issue_setup_token(Duration::from_secs(0), "tester")
        .await
        .expect_err("zero ttl should be rejected");
    assert!(matches!(invalid_ttl, ConfigError::InvalidField { .. }));

    let missing_before_issue = service
        .validate_setup_token("missing-token")
        .await
        .expect_err("missing active token should fail validation");
    assert!(matches!(
        missing_before_issue,
        ConfigError::SetupTokenMissing
    ));

    let token = service
        .issue_setup_token(Duration::from_secs(60), "tester")
        .await?;

    let validate_invalid = service
        .validate_setup_token("wrong-token")
        .await
        .expect_err("wrong token should fail validation");
    assert!(matches!(validate_invalid, ConfigError::SetupTokenInvalid));

    let consume_invalid = service
        .consume_setup_token("wrong-token")
        .await
        .expect_err("wrong token should fail consumption");
    assert!(matches!(consume_invalid, ConfigError::SetupTokenInvalid));

    service.validate_setup_token(&token.plaintext).await?;
    service.consume_setup_token(&token.plaintext).await?;

    let missing = service
        .validate_setup_token(&token.plaintext)
        .await
        .expect_err("consumed token should be missing");
    assert!(matches!(missing, ConfigError::SetupTokenMissing));
    Ok(())
}

#[tokio::test]
async fn config_service_new_with_session_round_trips_encrypted_secrets() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!(
                "skipping config_service_new_with_session_round_trips_encrypted_secrets: {err}"
            );
            return Ok(());
        }
    };
    let service = ConfigService::new_with_session(
        postgres.connection_string(),
        Some(DbSessionConfig::new(
            "integration-session",
            "session-secret",
        )),
    )
    .await?;

    service
        .apply_changeset(
            "tester",
            "session-secret-roundtrip",
            SettingsChangeset {
                secrets: vec![SecretPatch::Set {
                    name: "session-secret".to_string(),
                    value: "secret-value".to_string(),
                }],
                ..SettingsChangeset::default()
            },
        )
        .await?;

    assert_eq!(
        service.get_secret("session-secret").await?,
        Some("secret-value".to_string())
    );
    Ok(())
}

#[tokio::test]
async fn factory_reset_restores_default_auth_state() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping factory_reset_restores_default_auth_state: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let snapshot = service.snapshot().await?;

    let create = SettingsChangeset {
        app_profile: Some({
            let mut app = snapshot.app_profile.clone();
            app.mode = AppMode::Active;
            app
        }),
        engine_profile: None,
        fs_policy: None,
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "reset-key".to_string(),
            label: Some("reset".to_string()),
            enabled: Some(true),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
            secret: Some("reset-secret".to_string()),
            rate_limit: None,
        }],
        secrets: vec![SecretPatch::Set {
            name: "reset-secret".to_string(),
            value: "secret".to_string(),
        }],
    };
    service
        .apply_changeset("tester", "seed-reset-state", create)
        .await?;

    assert!(service.has_api_keys().await?);
    service.factory_reset().await?;

    let reset_snapshot = service.snapshot().await?;
    assert_eq!(reset_snapshot.app_profile.mode, AppMode::Setup);
    assert!(!service.has_api_keys().await?);
    assert!(service.get_secret("reset-secret").await?.is_none());
    Ok(())
}

#[tokio::test]
async fn config_service_round_trips_comprehensive_settings_update() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_round_trips_comprehensive_settings_update: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let snapshot = service.snapshot().await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let category_dir = PathBuf::from(&download_root).join("movies");
    let tag_dir = PathBuf::from(&library_root).join("television");
    fs::create_dir_all(&category_dir)?;
    fs::create_dir_all(&tag_dir)?;

    let mut app_profile = snapshot.app_profile.clone();
    app_profile.instance_name = "Integration Node".to_string();
    app_profile.mode = AppMode::Active;
    app_profile.auth_mode = AppAuthMode::ApiKey;
    app_profile.http_port += 11;
    app_profile.bind_addr = "127.0.0.1".parse::<IpAddr>()?;
    app_profile.local_networks = vec!["10.0.0.0/8".to_string(), "127.0.0.0/8".to_string()];
    app_profile.telemetry = TelemetryConfig {
        level: Some("debug".to_string()),
        format: Some("json".to_string()),
        otel_enabled: Some(true),
        otel_service_name: Some("revaer-config-it".to_string()),
        otel_endpoint: Some("http://127.0.0.1:4318".to_string()),
    };
    app_profile.label_policies = vec![
        LabelPolicy {
            kind: LabelKind::Category,
            name: "movies".to_string(),
            download_dir: Some(path_to_string(category_dir)),
            rate_limit_download_bps: Some(2_000_000),
            rate_limit_upload_bps: Some(1_000_000),
            queue_position: Some(3),
            auto_managed: Some(true),
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(7200),
            cleanup_seed_ratio_limit: Some(2.5),
            cleanup_seed_time_limit: Some(3600),
            cleanup_remove_data: Some(false),
        },
        LabelPolicy {
            kind: LabelKind::Tag,
            name: "tv".to_string(),
            download_dir: Some(path_to_string(tag_dir)),
            rate_limit_download_bps: Some(3_000_000),
            rate_limit_upload_bps: Some(2_000_000),
            queue_position: Some(5),
            auto_managed: Some(false),
            seed_ratio_limit: Some(2.0),
            seed_time_limit: Some(5400),
            cleanup_seed_ratio_limit: Some(3.0),
            cleanup_seed_time_limit: Some(1800),
            cleanup_remove_data: Some(true),
        },
    ];

    let mut engine_profile = snapshot.engine_profile.clone();
    engine_profile.listen_port = Some(6999);
    engine_profile.listen_interfaces = vec!["0.0.0.0:6999".to_string(), "[::]:6999".to_string()];
    engine_profile.ipv6_mode = "prefer_v6".to_string();
    engine_profile.anonymous_mode = Toggle::from(true);
    engine_profile.force_proxy = Toggle::from(true);
    engine_profile.prefer_rc4 = Toggle::from(true);
    engine_profile.allow_multiple_connections_per_ip = Toggle::from(true);
    engine_profile.enable_outgoing_utp = Toggle::from(false);
    engine_profile.enable_incoming_utp = Toggle::from(false);
    engine_profile.outgoing_port_min = Some(5000);
    engine_profile.outgoing_port_max = Some(5100);
    engine_profile.peer_dscp = Some(32);
    engine_profile.dht = false;
    engine_profile.encryption = "require".to_string();
    engine_profile.max_active = Some(9);
    engine_profile.max_download_bps = Some(123_456);
    engine_profile.max_upload_bps = Some(654_321);
    engine_profile.seed_ratio_limit = Some(1.25);
    engine_profile.seed_time_limit = Some(5400);
    engine_profile.connections_limit = Some(400);
    engine_profile.connections_limit_per_torrent = Some(50);
    engine_profile.unchoke_slots = Some(8);
    engine_profile.half_open_limit = Some(20);
    engine_profile.alt_speed = AltSpeedConfig {
        download_bps: Some(1111),
        upload_bps: Some(2222),
        schedule: Some(AltSpeedSchedule {
            days: vec![Weekday::Mon, Weekday::Fri],
            start_minutes: 30,
            end_minutes: 90,
        }),
    };
    engine_profile.stats_interval_ms = Some(12_000);
    engine_profile.sequential_default = true;
    engine_profile.auto_managed = Toggle::from(false);
    engine_profile.auto_manage_prefer_seeds = Toggle::from(true);
    engine_profile.dont_count_slow_torrents = Toggle::from(false);
    engine_profile.super_seeding = Toggle::from(true);
    engine_profile.choking_algorithm = "fixed_slots".to_string();
    engine_profile.seed_choking_algorithm = "round_robin".to_string();
    engine_profile.strict_super_seeding = Toggle::from(true);
    engine_profile.optimistic_unchoke_slots = Some(5);
    engine_profile.max_queued_disk_bytes = Some(65_536);
    engine_profile.resume_dir = resume_dir.clone();
    engine_profile.download_root = download_root.clone();
    engine_profile.storage_mode = "allocate".to_string();
    engine_profile.use_partfile = Toggle::from(false);
    engine_profile.disk_read_mode = Some("enable_os_cache".to_string());
    engine_profile.disk_write_mode = Some("write_through".to_string());
    engine_profile.verify_piece_hashes = Toggle::from(false);
    engine_profile.cache_size = Some(128);
    engine_profile.cache_expiry = Some(45);
    engine_profile.coalesce_reads = Toggle::from(false);
    engine_profile.coalesce_writes = Toggle::from(false);
    engine_profile.use_disk_cache_pool = Toggle::from(false);
    engine_profile.tracker = TrackerConfig {
        default: vec!["udp://tracker.example:80/announce".to_string()],
        extra: vec!["https://tracker-backup.example/announce".to_string()],
        replace: true,
        user_agent: Some("Revaer/Integration".to_string()),
        announce_ip: Some("198.51.100.5".to_string()),
        listen_interface: Some("eth0".to_string()),
        request_timeout_ms: Some(5000),
        announce_to_all: true,
        proxy: Some(TrackerProxyConfig {
            host: "proxy.example".to_string(),
            port: 8443,
            username_secret: Some("proxy-user".to_string()),
            password_secret: Some("proxy-pass".to_string()),
            kind: TrackerProxyType::Https,
            proxy_peers: true,
        }),
        ssl_cert: Some("cert.pem".to_string()),
        ssl_private_key: Some("key.pem".to_string()),
        ssl_ca_cert: Some("ca.pem".to_string()),
        ssl_tracker_verify: false,
        auth: Some(TrackerAuthConfig {
            username_secret: Some("tracker-user".to_string()),
            password_secret: Some("tracker-pass".to_string()),
            cookie_secret: Some("tracker-cookie".to_string()),
        }),
    };
    engine_profile.enable_lsd = Toggle::from(false);
    engine_profile.enable_upnp = Toggle::from(true);
    engine_profile.enable_natpmp = Toggle::from(false);
    engine_profile.enable_pex = Toggle::from(true);
    engine_profile.dht_bootstrap_nodes = vec![
        "router.bittorrent.com:6881".to_string(),
        "dht.transmissionbt.com:6881".to_string(),
    ];
    engine_profile.dht_router_nodes = vec!["router.utorrent.com:6881".to_string()];
    engine_profile.ip_filter = IpFilterConfig {
        cidrs: vec!["192.168.0.0/16".to_string(), "10.0.0.0/8".to_string()],
        blocklist_url: Some("https://example.invalid/blocklist.txt".to_string()),
        etag: Some("etag-wide".to_string()),
        last_updated_at: None,
        last_error: Some("last fetch failed".to_string()),
    };
    engine_profile.peer_classes = PeerClassesConfig {
        classes: vec![
            PeerClassConfig {
                id: 1,
                label: "seedbox".to_string(),
                download_priority: 7,
                upload_priority: 6,
                connection_limit_factor: 150,
                ignore_unchoke_slots: true,
            },
            PeerClassConfig {
                id: 2,
                label: "slow".to_string(),
                download_priority: 2,
                upload_priority: 1,
                connection_limit_factor: 75,
                ignore_unchoke_slots: false,
            },
        ],
        default: vec![1, 2],
    };

    let mut fs_policy = snapshot.fs_policy.clone();
    fs_policy.library_root = library_root.clone();
    fs_policy.extract = !snapshot.fs_policy.extract;
    fs_policy.par2 = "verify".to_string();
    fs_policy.flatten = !snapshot.fs_policy.flatten;
    fs_policy.move_mode = "move".to_string();
    fs_policy.cleanup_keep = vec!["**/*.mkv".to_string(), "**/*.srt".to_string()];
    fs_policy.cleanup_drop = vec!["**/sample/**".to_string(), "**/*.nfo".to_string()];
    fs_policy.chmod_file = Some("640".to_string());
    fs_policy.chmod_dir = Some("750".to_string());
    fs_policy.owner = Some("media".to_string());
    fs_policy.group = Some("media".to_string());
    fs_policy.umask = Some("027".to_string());
    fs_policy.allow_paths = vec![download_root.clone(), library_root.clone()];

    let applied = service
        .apply_changeset(
            "tester",
            "comprehensive-update",
            SettingsChangeset {
                app_profile: Some(app_profile.clone()),
                engine_profile: Some(engine_profile.clone()),
                fs_policy: Some(fs_policy.clone()),
                api_keys: vec![ApiKeyPatch::Upsert {
                    key_id: "wide-key".to_string(),
                    label: Some("wide".to_string()),
                    enabled: Some(true),
                    expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(30)),
                    secret: Some("wide-secret".to_string()),
                    rate_limit: Some(Some(ApiKeyRateLimit {
                        burst: 20,
                        replenish_period: Duration::from_secs(15),
                    })),
                }],
                secrets: vec![SecretPatch::Set {
                    name: "wide-secret".to_string(),
                    value: "shared-secret".to_string(),
                }],
            },
        )
        .await?;

    assert!(applied.revision > snapshot.revision);
    assert!(applied.app_profile.is_some());
    assert!(applied.engine_profile.is_some());
    assert!(applied.fs_policy.is_some());

    let refreshed = service.snapshot().await?;
    assert_eq!(
        refreshed.app_profile.instance_name,
        app_profile.instance_name
    );
    assert_eq!(refreshed.app_profile.mode, AppMode::Active);
    assert_eq!(refreshed.app_profile.auth_mode, AppAuthMode::ApiKey);
    assert_eq!(refreshed.app_profile.http_port, app_profile.http_port);
    assert_eq!(refreshed.app_profile.bind_addr, app_profile.bind_addr);
    assert_eq!(refreshed.app_profile.telemetry, app_profile.telemetry);
    assert_eq!(
        refreshed.app_profile.label_policies,
        app_profile.label_policies
    );
    assert_eq!(
        refreshed.engine_profile.listen_port,
        engine_profile.listen_port
    );
    assert_eq!(
        refreshed.engine_profile.listen_interfaces,
        engine_profile.listen_interfaces
    );
    assert_eq!(refreshed.engine_profile.ipv6_mode, engine_profile.ipv6_mode);
    assert_eq!(refreshed.engine_profile.tracker, engine_profile.tracker);
    assert_eq!(refreshed.engine_profile.alt_speed, engine_profile.alt_speed);
    assert_eq!(refreshed.engine_profile.ip_filter, engine_profile.ip_filter);
    assert_eq!(
        refreshed.engine_profile.peer_classes,
        engine_profile.peer_classes
    );
    assert_eq!(refreshed.fs_policy, fs_policy);
    assert_eq!(
        service.get_secret("wide-secret").await?,
        Some("shared-secret".to_string())
    );
    let auth = service
        .authenticate_api_key("wide-key", "wide-secret")
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected wide-key authentication"))?;
    assert_eq!(auth.label, Some("wide".to_string()));
    let rate_limit = auth
        .rate_limit
        .ok_or_else(|| anyhow::anyhow!("expected API key rate limit"))?;
    assert_eq!(rate_limit.burst, 20);
    assert_eq!(rate_limit.replenish_period, Duration::from_secs(15));

    Ok(())
}

#[tokio::test]
async fn config_service_rejects_immutable_and_invalid_mutations() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_rejects_immutable_and_invalid_mutations: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let snapshot = service.snapshot().await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let mut freeze = snapshot.app_profile.clone();
    freeze.immutable_keys = vec![
        "app_profile.instance_name".to_string(),
        "engine_profile.listen_port".to_string(),
        "fs_policy.library_root".to_string(),
        "auth_api_keys.secret".to_string(),
        "settings_secret.webhook_token".to_string(),
    ];
    service
        .apply_changeset(
            "tester",
            "freeze-fields",
            SettingsChangeset {
                app_profile: Some(freeze),
                ..SettingsChangeset::default()
            },
        )
        .await?;

    let mut app_profile = service.get_app_profile().await?;
    app_profile.instance_name = "blocked".to_string();
    let err = service
        .apply_changeset(
            "tester",
            "immutable-app",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected immutable app field");
    assert!(matches!(
        err,
        ConfigError::ImmutableField { section, field }
        if section == "app_profile" && field == "instance_name"
    ));

    let mut engine_profile = service.get_engine_profile().await?;
    engine_profile.listen_port = Some(7001);
    engine_profile.resume_dir = resume_dir;
    engine_profile.download_root = download_root;
    let err = service
        .apply_changeset(
            "tester",
            "immutable-engine",
            SettingsChangeset {
                engine_profile: Some(engine_profile),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected immutable engine field");
    assert!(matches!(
        err,
        ConfigError::ImmutableField { section, field }
        if section == "engine_profile" && field == "listen_port"
    ));

    let mut fs_policy = service.get_fs_policy().await?;
    fs_policy.library_root = library_root;
    let err = service
        .apply_changeset(
            "tester",
            "immutable-fs",
            SettingsChangeset {
                fs_policy: Some(fs_policy),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected immutable fs field");
    assert!(matches!(
        err,
        ConfigError::ImmutableField { section, field }
        if section == "fs_policy" && field == "library_root"
    ));

    let err = service
        .apply_changeset(
            "tester",
            "immutable-secret",
            SettingsChangeset {
                secrets: vec![SecretPatch::Set {
                    name: "webhook_token".to_string(),
                    value: "secret".to_string(),
                }],
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected immutable secret field");
    assert!(matches!(
        err,
        ConfigError::ImmutableField { section, field }
        if section == "settings_secret" && field == "webhook_token"
    ));

    let err = service
        .apply_changeset(
            "tester",
            "immutable-api-secret",
            SettingsChangeset {
                api_keys: vec![ApiKeyPatch::Upsert {
                    key_id: "immutable-key".to_string(),
                    label: Some("immutable".to_string()),
                    enabled: Some(true),
                    expires_at: None,
                    secret: Some("blocked-secret".to_string()),
                    rate_limit: None,
                }],
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected immutable API key field");
    assert!(matches!(
        err,
        ConfigError::ImmutableField { section, field }
        if section == "auth_api_keys" && field == "secret"
    ));

    let err = service
        .apply_changeset(
            "tester",
            "missing-secret",
            SettingsChangeset {
                api_keys: vec![ApiKeyPatch::Upsert {
                    key_id: "new-key".to_string(),
                    label: Some("new".to_string()),
                    enabled: Some(true),
                    expires_at: None,
                    secret: None,
                    rate_limit: None,
                }],
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected missing API key secret");
    assert!(matches!(
        err,
        ConfigError::InvalidField { section, field, reason, .. }
        if section == "auth_api_keys" && field == "secret" && reason == "required when creating a new API key"
    ));

    let missing_dir = server_root()?.join(format!("missing-{}", Uuid::new_v4()));
    let missing_path = path_to_string(missing_dir.clone());

    let mut invalid_engine = service.get_engine_profile().await?;
    invalid_engine.download_root = missing_path.clone();
    let err = service
        .apply_changeset(
            "tester",
            "invalid-engine-path",
            SettingsChangeset {
                engine_profile: Some(invalid_engine),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected invalid engine path");
    assert!(matches!(
        err,
        ConfigError::InvalidField { section, field, reason, .. }
        if section == "engine_profile" && field == "download_root" && reason == "path must exist"
    ));

    let mut invalid_fs = service.get_fs_policy().await?;
    invalid_fs.allow_paths = vec![missing_path];
    let err = service
        .apply_changeset(
            "tester",
            "invalid-fs-path",
            SettingsChangeset {
                fs_policy: Some(invalid_fs),
                ..SettingsChangeset::default()
            },
        )
        .await
        .expect_err("expected invalid fs path");
    assert!(matches!(
        err,
        ConfigError::InvalidField { section, field, reason, .. }
        if section == "fs_policy" && field == "allow_paths" && reason == "path must exist"
    ));

    Ok(())
}

#[tokio::test]
async fn config_service_handles_auth_edge_cases_and_binary_secrets() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_handles_auth_edge_cases_and_binary_secrets: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    service
        .apply_changeset(
            "tester",
            "seed-auth",
            SettingsChangeset {
                api_keys: vec![ApiKeyPatch::Upsert {
                    key_id: "edge-key".to_string(),
                    label: Some("edge".to_string()),
                    enabled: Some(true),
                    expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
                    secret: Some("edge-secret".to_string()),
                    rate_limit: Some(Some(ApiKeyRateLimit {
                        burst: 4,
                        replenish_period: Duration::from_secs(20),
                    })),
                }],
                ..SettingsChangeset::default()
            },
        )
        .await?;

    data_config::update_api_key_enabled(service.pool(), "edge-key", false).await?;
    assert!(
        service
            .authenticate_api_key("edge-key", "edge-secret")
            .await?
            .is_none()
    );

    data_config::update_api_key_enabled(service.pool(), "edge-key", true).await?;
    data_config::update_api_key_expires_at(service.pool(), "edge-key", None).await?;
    assert!(
        service
            .authenticate_api_key("edge-key", "edge-secret")
            .await?
            .is_none()
    );

    data_config::update_api_key_expires_at(
        service.pool(),
        "edge-key",
        Some(chrono::Utc::now() - chrono::Duration::minutes(1)),
    )
    .await?;
    assert!(
        service
            .authenticate_api_key("edge-key", "edge-secret")
            .await?
            .is_none()
    );

    data_config::update_api_key_expires_at(
        service.pool(),
        "edge-key",
        Some(chrono::Utc::now() + chrono::Duration::minutes(5)),
    )
    .await?;
    data_config::update_api_key_rate_limit(service.pool(), "edge-key", Some(-1), Some(10)).await?;
    let err = service
        .authenticate_api_key("edge-key", "edge-secret")
        .await
        .expect_err("expected invalid rate limit error");
    assert!(matches!(
        err,
        ConfigError::InvalidField { section, field, reason, .. }
        if section == "api_keys" && field == "rate_limit.burst" && reason == "invalid rate limit burst"
    ));

    data_config::upsert_secret(service.pool(), "binary-secret", &[0xff], "tester").await?;
    let err = service
        .get_secret("binary-secret")
        .await
        .expect_err("expected invalid UTF-8 secret");
    assert!(matches!(
        err,
        ConfigError::InvalidField { section, field, reason, .. }
        if section == "settings_secret" && field == "binary-secret" && reason == "payload is not valid UTF-8"
    ));

    Ok(())
}

#[tokio::test]
async fn settings_stream_and_watcher_cover_notifications_and_polling() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!(
                "skipping settings_stream_and_watcher_cover_notifications_and_polling: {err}"
            );
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;

    let mut stream = service.subscribe_changes().await?;
    notify_settings_change(&service, "mystery:42:DELETE").await?;
    let change = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("settings stream closed"))??;
    assert_eq!(change.table, "mystery");
    assert_eq!(change.revision, 42);
    assert_eq!(change.operation, "DELETE");
    assert!(matches!(change.payload, SettingsPayload::None));

    notify_settings_change(&service, "invalid-payload").await?;
    let err = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("settings stream closed"))?
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected notification parsing error"))?;
    assert!(matches!(
        err,
        ConfigError::NotificationPayloadMissingRevision
    ));

    let (snapshot, mut watcher) = service.watch_settings(Duration::from_millis(10)).await?;
    watcher.disable_listen();

    let mut app_profile = snapshot.app_profile.clone();
    app_profile.instance_name = "Poll fallback".to_string();
    let applied = service
        .apply_changeset(
            "tester",
            "poll-fallback",
            SettingsChangeset {
                app_profile: Some(app_profile),
                ..SettingsChangeset::default()
            },
        )
        .await?;

    let updated = tokio::time::timeout(Duration::from_secs(5), watcher.next()).await??;
    assert!(updated.revision >= applied.revision);
    assert_eq!(updated.app_profile.instance_name, "Poll fallback");

    Ok(())
}

#[tokio::test]
async fn settings_stream_maps_known_tables_to_typed_payloads() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping settings_stream_maps_known_tables_to_typed_payloads: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let mut stream = service.subscribe_changes().await?;
    let snapshot = service.snapshot().await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let mut app_profile = snapshot.app_profile.clone();
    app_profile.instance_name = "Typed payload app".to_string();
    service
        .apply_changeset(
            "tester",
            "typed-app-payload",
            SettingsChangeset {
                app_profile: Some(app_profile.clone()),
                ..SettingsChangeset::default()
            },
        )
        .await?;
    let app_change = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("settings stream closed"))??;
    assert_eq!(app_change.table, "app_profile");
    match app_change.payload {
        SettingsPayload::AppProfile(payload) => {
            assert_eq!(payload.instance_name, app_profile.instance_name);
            assert_eq!(payload.mode, app_profile.mode);
        }
        other => return Err(anyhow::anyhow!("unexpected app payload: {other:?}")),
    }

    let mut engine_profile = snapshot.engine_profile.clone();
    engine_profile.listen_port = Some(snapshot.engine_profile.listen_port.unwrap_or(6881) + 1);
    engine_profile.dht = !snapshot.engine_profile.dht;
    engine_profile.resume_dir = resume_dir;
    engine_profile.download_root = download_root;
    service
        .apply_changeset(
            "tester",
            "typed-engine-payload",
            SettingsChangeset {
                engine_profile: Some(engine_profile.clone()),
                ..SettingsChangeset::default()
            },
        )
        .await?;
    let engine_change = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("settings stream closed"))??;
    assert_eq!(engine_change.table, "engine_profile");
    match engine_change.payload {
        SettingsPayload::EngineProfile(payload) => {
            assert_eq!(payload.dht, engine_profile.dht);
            assert_eq!(payload.download_root, engine_profile.download_root);
            assert_eq!(payload.resume_dir, engine_profile.resume_dir);
        }
        other => return Err(anyhow::anyhow!("unexpected engine payload: {other:?}")),
    }

    let mut fs_policy = snapshot.fs_policy.clone();
    fs_policy.library_root = library_root;
    fs_policy.flatten = !snapshot.fs_policy.flatten;
    service
        .apply_changeset(
            "tester",
            "typed-fs-payload",
            SettingsChangeset {
                fs_policy: Some(fs_policy.clone()),
                ..SettingsChangeset::default()
            },
        )
        .await?;
    let fs_change = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("settings stream closed"))??;
    assert_eq!(fs_change.table, "fs_policy");
    match fs_change.payload {
        SettingsPayload::FsPolicy(payload) => {
            assert_eq!(payload.library_root, fs_policy.library_root);
            assert_eq!(payload.flatten, fs_policy.flatten);
        }
        other => return Err(anyhow::anyhow!("unexpected fs payload: {other:?}")),
    }

    Ok(())
}

#[tokio::test]
async fn apply_empty_changeset_does_not_advance_revision() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping apply_empty_changeset_does_not_advance_revision: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let before = service.snapshot().await?;

    let applied = service
        .apply_changeset("tester", "noop", SettingsChangeset::default())
        .await?;
    let after = service.snapshot().await?;

    assert_eq!(applied.revision, before.revision);
    assert!(applied.app_profile.is_none());
    assert!(applied.engine_profile.is_none());
    assert!(applied.fs_policy.is_none());
    assert_eq!(after.revision, before.revision);

    Ok(())
}

#[tokio::test]
async fn config_service_clears_optional_settings_and_label_policies() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_service_clears_optional_settings_and_label_policies: {err}");
            return Ok(());
        }
    };
    let service = ConfigService::new(postgres.connection_string()).await?;
    let snapshot = service.snapshot().await?;
    let (download_root, resume_dir, library_root) = build_temp_paths()?;

    let category_dir = PathBuf::from(&library_root).join("movies");
    fs::create_dir_all(&category_dir)?;

    let mut seeded_app = snapshot.app_profile.clone();
    seeded_app.mode = AppMode::Active;
    seeded_app.telemetry = TelemetryConfig {
        level: Some("debug".to_string()),
        format: Some("json".to_string()),
        otel_enabled: Some(true),
        otel_service_name: Some("revaer-clear-test".to_string()),
        otel_endpoint: Some("http://127.0.0.1:4318".to_string()),
    };
    seeded_app.label_policies = vec![LabelPolicy {
        kind: LabelKind::Category,
        name: "movies".to_string(),
        download_dir: Some(path_to_string(category_dir)),
        rate_limit_download_bps: Some(1_000_000),
        rate_limit_upload_bps: Some(500_000),
        queue_position: Some(2),
        auto_managed: Some(true),
        seed_ratio_limit: Some(1.5),
        seed_time_limit: Some(3600),
        cleanup_seed_ratio_limit: Some(2.5),
        cleanup_seed_time_limit: Some(1800),
        cleanup_remove_data: Some(false),
    }];

    let mut seeded_engine = snapshot.engine_profile.clone();
    seeded_engine.download_root = download_root.clone();
    seeded_engine.resume_dir = resume_dir.clone();
    seeded_engine.listen_interfaces = vec!["0.0.0.0:6889".to_string()];
    seeded_engine.alt_speed = AltSpeedConfig {
        download_bps: Some(1234),
        upload_bps: Some(5678),
        schedule: Some(AltSpeedSchedule {
            days: vec![Weekday::Mon, Weekday::Fri],
            start_minutes: 60,
            end_minutes: 120,
        }),
    };
    seeded_engine.tracker = TrackerConfig {
        default: vec!["udp://tracker.example:80/announce".to_string()],
        extra: vec!["https://backup.example/announce".to_string()],
        replace: true,
        user_agent: Some("Revaer/Clear".to_string()),
        announce_ip: Some("198.51.100.5".to_string()),
        listen_interface: Some("eth0".to_string()),
        request_timeout_ms: Some(5000),
        announce_to_all: true,
        proxy: Some(TrackerProxyConfig {
            host: "proxy.example".to_string(),
            port: 8443,
            username_secret: Some("proxy-user".to_string()),
            password_secret: Some("proxy-pass".to_string()),
            kind: TrackerProxyType::Https,
            proxy_peers: true,
        }),
        ssl_cert: Some("cert.pem".to_string()),
        ssl_private_key: Some("key.pem".to_string()),
        ssl_ca_cert: Some("ca.pem".to_string()),
        ssl_tracker_verify: false,
        auth: Some(TrackerAuthConfig {
            username_secret: Some("tracker-user".to_string()),
            password_secret: Some("tracker-pass".to_string()),
            cookie_secret: Some("tracker-cookie".to_string()),
        }),
    };
    seeded_engine.dht_bootstrap_nodes = vec!["router.bittorrent.com:6881".to_string()];
    seeded_engine.dht_router_nodes = vec!["router.utorrent.com:6881".to_string()];
    seeded_engine.ip_filter = IpFilterConfig {
        cidrs: vec!["10.0.0.0/8".to_string()],
        blocklist_url: Some("https://example.invalid/blocklist.txt".to_string()),
        etag: Some("etag-value".to_string()),
        last_updated_at: None,
        last_error: Some("fetch failed".to_string()),
    };
    seeded_engine.peer_classes = PeerClassesConfig {
        classes: vec![PeerClassConfig {
            id: 1,
            label: "seedbox".to_string(),
            download_priority: 6,
            upload_priority: 5,
            connection_limit_factor: 150,
            ignore_unchoke_slots: true,
        }],
        default: vec![1],
    };

    let mut seeded_fs = snapshot.fs_policy.clone();
    seeded_fs.library_root = library_root.clone();
    seeded_fs.cleanup_keep = vec!["**/*.mkv".to_string()];
    seeded_fs.cleanup_drop = vec!["**/*.nfo".to_string()];
    seeded_fs.chmod_file = Some("640".to_string());
    seeded_fs.chmod_dir = Some("750".to_string());
    seeded_fs.owner = Some("media".to_string());
    seeded_fs.group = Some("media".to_string());
    seeded_fs.umask = Some("027".to_string());
    seeded_fs.allow_paths = vec![download_root.clone(), library_root.clone()];

    service
        .apply_changeset(
            "tester",
            "seed-clearable-state",
            SettingsChangeset {
                app_profile: Some(seeded_app),
                engine_profile: Some(seeded_engine),
                fs_policy: Some(seeded_fs),
                ..SettingsChangeset::default()
            },
        )
        .await?;

    let mut cleared_app = service.get_app_profile().await?;
    cleared_app.telemetry = TelemetryConfig::default();
    cleared_app.label_policies = Vec::new();

    let mut cleared_engine = service.get_engine_profile().await?;
    cleared_engine.listen_interfaces = Vec::new();
    cleared_engine.alt_speed = AltSpeedConfig::default();
    cleared_engine.tracker = TrackerConfig::default();
    cleared_engine.dht_bootstrap_nodes = Vec::new();
    cleared_engine.dht_router_nodes = Vec::new();
    cleared_engine.ip_filter = IpFilterConfig::default();
    cleared_engine.peer_classes = PeerClassesConfig::default();

    let mut cleared_fs = service.get_fs_policy().await?;
    cleared_fs.cleanup_keep = Vec::new();
    cleared_fs.cleanup_drop = Vec::new();
    cleared_fs.chmod_file = None;
    cleared_fs.chmod_dir = None;
    cleared_fs.owner = None;
    cleared_fs.group = None;
    cleared_fs.umask = None;
    cleared_fs.allow_paths = vec![library_root.clone()];

    service
        .apply_changeset(
            "tester",
            "clear-optional-state",
            SettingsChangeset {
                app_profile: Some(cleared_app),
                engine_profile: Some(cleared_engine),
                fs_policy: Some(cleared_fs),
                ..SettingsChangeset::default()
            },
        )
        .await?;

    let refreshed = service.snapshot().await?;
    assert!(refreshed.app_profile.label_policies.is_empty());
    assert_eq!(refreshed.app_profile.telemetry, TelemetryConfig::default());
    assert!(refreshed.engine_profile.listen_interfaces.is_empty());
    assert_eq!(
        refreshed.engine_profile.alt_speed,
        AltSpeedConfig::default()
    );
    assert_eq!(refreshed.engine_profile.tracker, TrackerConfig::default());
    assert!(refreshed.engine_profile.dht_bootstrap_nodes.is_empty());
    assert!(refreshed.engine_profile.dht_router_nodes.is_empty());
    assert_eq!(
        refreshed.engine_profile.ip_filter,
        IpFilterConfig::default()
    );
    assert_eq!(
        refreshed.engine_profile.peer_classes,
        PeerClassesConfig::default()
    );
    assert!(refreshed.fs_policy.cleanup_keep.is_empty());
    assert!(refreshed.fs_policy.cleanup_drop.is_empty());
    assert_eq!(refreshed.fs_policy.chmod_file, None);
    assert_eq!(refreshed.fs_policy.chmod_dir, None);
    assert_eq!(refreshed.fs_policy.owner, None);
    assert_eq!(refreshed.fs_policy.group, None);
    assert_eq!(refreshed.fs_policy.umask, None);
    assert_eq!(refreshed.fs_policy.allow_paths, vec![library_root]);

    Ok(())
}

fn build_temp_paths() -> anyhow::Result<(String, String, String)> {
    let base = server_root()?.join(format!("revaer-config-{}", Uuid::new_v4()));
    let download_root = base.join("downloads");
    let resume_dir = base.join("resume");
    let library_root = base.join("library");
    fs::create_dir_all(&download_root)?;
    fs::create_dir_all(&resume_dir)?;
    fs::create_dir_all(&library_root)?;
    Ok((
        path_to_string(download_root),
        path_to_string(resume_dir),
        path_to_string(library_root),
    ))
}

fn server_root() -> anyhow::Result<PathBuf> {
    let root = repo_root().join(".server_root");
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        if ancestor.join("AGENT.md").is_file() {
            return ancestor.to_path_buf();
        }
    }
    manifest_dir
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

async fn notify_settings_change(service: &ConfigService, payload: &str) -> anyhow::Result<()> {
    data_config::notify_settings_changed(service.pool(), payload).await?;
    Ok(())
}
