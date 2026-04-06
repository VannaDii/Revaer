use chrono::{Duration as ChronoDuration, Utc};
use revaer_data::config::{
    AltSpeedUpdate, AppLabelPoliciesUpdate, EngineProfileRow, EngineProfileUpdate, FsArrayField,
    FsBooleanField, FsOptionalStringField, FsStringField, IpFilterUpdate, NatToggleSet, NewApiKey,
    NewSetupToken, PeerClassesUpdate, PrivacyToggleSet, QueuePolicySet, SeedingToggleSet,
    StorageToggleSet, TrackerAnnouncePolicy, TrackerConfigUpdate, TrackerProxyPolicy,
    TrackerTlsPolicy, bump_app_profile_version, bump_revision, cleanup_expired_setup_tokens,
    delete_api_key, delete_secret, factory_reset, fetch_active_setup_token, fetch_api_key_auth,
    fetch_api_key_hash, fetch_api_keys, fetch_app_label_policies, fetch_app_profile_row,
    fetch_engine_profile_row, fetch_fs_policy_row, fetch_revision, fetch_secret_by_name,
    insert_api_key, insert_setup_token, invalidate_active_setup_tokens, mark_setup_token_consumed,
    replace_app_label_policies, run_migrations, set_engine_alt_speed, set_engine_ip_filter,
    set_engine_list_values, set_peer_classes, set_tracker_config, update_api_key_enabled,
    update_api_key_expires_at, update_api_key_hash, update_api_key_label,
    update_api_key_rate_limit, update_app_auth_mode, update_app_bind_addr, update_app_http_port,
    update_app_immutable_keys, update_app_instance_name, update_app_local_networks,
    update_app_mode, update_app_telemetry, update_engine_profile, update_fs_array_field,
    update_fs_boolean_field, update_fs_optional_string_field, update_fs_string_field,
    upsert_secret,
};
use revaer_test_support::postgres::start_postgres;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";

fn engine_update_from_row(row: &EngineProfileRow) -> EngineProfileUpdate<'_> {
    EngineProfileUpdate {
        id: row.id,
        implementation: &row.implementation,
        listen_port: row.listen_port,
        dht: row.dht,
        encryption: &row.encryption,
        max_active: row.max_active,
        max_download_bps: row.max_download_bps,
        max_upload_bps: row.max_upload_bps,
        seed_ratio_limit: row.seed_ratio_limit,
        seed_time_limit: row.seed_time_limit,
        queue: row.queue,
        seeding: row.seeding,
        choking_algorithm: &row.choking_algorithm,
        seed_choking_algorithm: &row.seed_choking_algorithm,
        optimistic_unchoke_slots: row.optimistic_unchoke_slots,
        max_queued_disk_bytes: row.max_queued_disk_bytes,
        resume_dir: &row.resume_dir,
        download_root: &row.download_root,
        storage_mode: &row.storage_mode,
        storage: row.storage,
        disk_read_mode: row.disk_read_mode.as_deref(),
        disk_write_mode: row.disk_write_mode.as_deref(),
        verify_piece_hashes: row.verify_piece_hashes,
        cache_size: row.cache_size,
        cache_expiry: row.cache_expiry,
        nat: row.nat,
        ipv6_mode: &row.ipv6_mode,
        privacy: row.privacy,
        outgoing_port_min: row.outgoing_port_min,
        outgoing_port_max: row.outgoing_port_max,
        peer_dscp: row.peer_dscp,
        connections_limit: row.connections_limit,
        connections_limit_per_torrent: row.connections_limit_per_torrent,
        unchoke_slots: row.unchoke_slots,
        half_open_limit: row.half_open_limit,
        stats_interval_ms: row.stats_interval_ms,
    }
}

#[tokio::test]
async fn config_wrappers_round_trip() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_wrappers_round_trip: {err}");
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;

    run_migrations(&pool).await?;

    let app_id = Uuid::parse_str(APP_PROFILE_ID)?;
    let engine_id = Uuid::parse_str(ENGINE_PROFILE_ID)?;
    let fs_id = Uuid::parse_str(FS_POLICY_ID)?;

    let revision = fetch_revision(&pool).await?;
    let bumped = bump_revision(&pool, "app_profile").await?;
    assert!(bumped >= revision);

    update_app_instance_name(&pool, app_id, "Revaer Test").await?;
    update_app_mode(&pool, app_id, "active").await?;
    update_app_auth_mode(&pool, app_id, "api_key").await?;
    update_app_http_port(&pool, app_id, 7071).await?;
    update_app_bind_addr(&pool, app_id, "127.0.0.1").await?;
    update_app_telemetry(
        &pool,
        app_id,
        Some("info"),
        Some("json"),
        Some(true),
        Some("revaer-test"),
        Some("http://otel.local"),
    )
    .await?;
    update_app_local_networks(&pool, app_id, &["127.0.0.0/8".to_string()]).await?;
    update_app_immutable_keys(&pool, app_id, &["app_profile.mode".to_string()]).await?;
    bump_app_profile_version(&pool, app_id).await?;

    let app_row = fetch_app_profile_row(&pool, app_id).await?;
    assert_eq!(app_row.instance_name, "Revaer Test");
    assert_eq!(app_row.http_port, 7071);
    assert_eq!(app_row.bind_addr, "127.0.0.1/32");

    let kinds = vec!["category".to_string()];
    let names = vec!["movies".to_string()];
    let download_dirs = vec![Some("/data/movies".to_string())];
    let rate_limit_download_bps = vec![Some(1_024)];
    let rate_limit_upload_bps = vec![Some(2_048)];
    let queue_positions = vec![Some(2)];
    let auto_managed = vec![Some(true)];
    let seed_ratio_limits = vec![Some(1.5)];
    let seed_time_limits = vec![Some(3_600)];
    let cleanup_seed_ratio_limits = vec![Some(2.0)];
    let cleanup_seed_time_limits = vec![Some(1_800)];
    let cleanup_remove_data = vec![Some(false)];
    let label_update = AppLabelPoliciesUpdate {
        kinds: &kinds,
        names: &names,
        download_dirs: &download_dirs,
        rate_limit_download_bps: &rate_limit_download_bps,
        rate_limit_upload_bps: &rate_limit_upload_bps,
        queue_positions: &queue_positions,
        auto_managed: &auto_managed,
        seed_ratio_limits: &seed_ratio_limits,
        seed_time_limits: &seed_time_limits,
        cleanup_seed_ratio_limits: &cleanup_seed_ratio_limits,
        cleanup_seed_time_limits: &cleanup_seed_time_limits,
        cleanup_remove_data: &cleanup_remove_data,
    };
    replace_app_label_policies(&pool, app_id, &label_update).await?;
    let policies = fetch_app_label_policies(&pool, app_id).await?;
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0].name, "movies");

    update_fs_string_field(&pool, fs_id, FsStringField::LibraryRoot, "/library").await?;
    update_fs_boolean_field(&pool, fs_id, FsBooleanField::Extract, true).await?;
    update_fs_array_field(
        &pool,
        fs_id,
        FsArrayField::AllowPaths,
        &["/library".to_string()],
    )
    .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::ChmodFile, Some("644"))
        .await?;
    let fs_row = fetch_fs_policy_row(&pool, fs_id).await?;
    assert_eq!(fs_row.library_root, "/library");
    assert!(fs_row.extract);
    assert_eq!(fs_row.allow_paths, vec!["/library".to_string()]);
    assert_eq!(fs_row.chmod_file.as_deref(), Some("644"));

    let engine_row = fetch_engine_profile_row(&pool, engine_id).await?;
    let mut engine_update = engine_update_from_row(&engine_row);
    engine_update.max_active = Some(engine_row.max_active.unwrap_or(1).saturating_add(1));
    engine_update.seeding = SeedingToggleSet::from_flags([true, false, false]);
    engine_update.queue = QueuePolicySet::from_flags([true, true, false]);
    engine_update.storage = StorageToggleSet::from_flags([true, true, false, false]);
    engine_update.nat = NatToggleSet::from_flags([true, false, true, false]);
    engine_update.privacy = PrivacyToggleSet::from_flags([true, false, false, false, true, false]);
    update_engine_profile(&pool, &engine_update).await?;

    set_engine_list_values(
        &pool,
        engine_id,
        "dht_bootstrap_nodes",
        &["router.example:6881".to_string()],
    )
    .await?;

    let ip_filter_cidrs = vec!["10.0.0.0/8".to_string()];
    let ip_filter_update = IpFilterUpdate {
        blocklist_url: Some("https://blocklist.example"),
        etag: Some("etag"),
        last_updated_at: Some(Utc::now()),
        last_error: None,
        cidrs: &ip_filter_cidrs,
    };
    set_engine_ip_filter(&pool, engine_id, &ip_filter_update).await?;

    let alt_speed_days = vec!["mon".to_string()];
    let alt_speed_update = AltSpeedUpdate {
        download_bps: Some(500),
        upload_bps: Some(250),
        schedule_start_minutes: Some(60),
        schedule_end_minutes: Some(120),
        days: &alt_speed_days,
    };
    set_engine_alt_speed(&pool, engine_id, &alt_speed_update).await?;

    let tracker_default = vec!["https://tracker.example/announce".to_string()];
    let tracker_extra = vec!["https://tracker.extra/announce".to_string()];
    let tracker_update = TrackerConfigUpdate {
        user_agent: Some("RevaerTest"),
        announce_ip: None,
        listen_interface: None,
        request_timeout_ms: Some(1_000),
        announce: TrackerAnnouncePolicy {
            announce_to_all: true,
            replace_trackers: false,
        },
        proxy_host: None,
        proxy_port: None,
        proxy_kind: None,
        proxy_username_secret: None,
        proxy_password_secret: None,
        proxy: TrackerProxyPolicy { proxy_peers: false },
        ssl_cert: None,
        ssl_private_key: None,
        ssl_ca_cert: None,
        tls: TrackerTlsPolicy { verify: false },
        auth_username_secret: None,
        auth_password_secret: None,
        auth_cookie_secret: None,
        default_urls: &tracker_default,
        extra_urls: &tracker_extra,
    };
    set_tracker_config(&pool, engine_id, &tracker_update).await?;

    let peer_class_ids = vec![1];
    let peer_labels = vec!["fast".to_string()];
    let peer_download_priorities = vec![1];
    let peer_upload_priorities = vec![1];
    let peer_connection_limits = vec![100];
    let peer_ignore = vec![false];
    let peer_defaults = vec![1];
    let peer_update = PeerClassesUpdate {
        class_ids: &peer_class_ids,
        labels: &peer_labels,
        download_priorities: &peer_download_priorities,
        upload_priorities: &peer_upload_priorities,
        connection_limit_factors: &peer_connection_limits,
        ignore_unchoke_slots: &peer_ignore,
        default_class_ids: &peer_defaults,
    };
    set_peer_classes(&pool, engine_id, &peer_update).await?;

    let refreshed_engine = fetch_engine_profile_row(&pool, engine_id).await?;
    assert_eq!(refreshed_engine.ip_filter_cidrs, ip_filter_cidrs);
    assert_eq!(refreshed_engine.tracker_default_urls, tracker_default);
    assert_eq!(
        refreshed_engine.dht_bootstrap_nodes,
        vec!["router.example:6881".to_string()]
    );

    upsert_secret(&pool, "webhook", b"payload", "tester").await?;
    let secret = fetch_secret_by_name(&pool, "webhook").await?;
    assert!(secret.is_some());
    let removed = delete_secret(&pool, "webhook").await?;
    assert_eq!(removed, 1);
    let secret = fetch_secret_by_name(&pool, "webhook").await?;
    assert!(secret.is_none());

    cleanup_expired_setup_tokens(&pool).await?;
    let setup_token = NewSetupToken {
        token_hash: "token-hash",
        expires_at: Utc::now() + ChronoDuration::minutes(5),
        issued_by: "tester",
    };
    insert_setup_token(&pool, &setup_token).await?;
    let active = fetch_active_setup_token(&pool).await?;
    let active = active.ok_or_else(|| anyhow::anyhow!("expected setup token"))?;
    mark_setup_token_consumed(&pool, active.id).await?;
    invalidate_active_setup_tokens(&pool).await?;

    let key_id = "ci-key";
    let new_key = NewApiKey {
        key_id,
        hash: "hash",
        label: Some("ci"),
        enabled: true,
        burst: Some(10),
        per_seconds: Some(60),
        expires_at: None,
    };
    insert_api_key(&pool, &new_key).await?;
    let keys = fetch_api_keys(&pool).await?;
    assert!(keys.iter().any(|key| key.key_id == key_id));
    update_api_key_label(&pool, key_id, Some("ci-updated")).await?;
    update_api_key_enabled(&pool, key_id, false).await?;
    update_api_key_rate_limit(&pool, key_id, Some(5), Some(120)).await?;
    update_api_key_expires_at(&pool, key_id, Some(Utc::now() + ChronoDuration::hours(1))).await?;
    update_api_key_hash(&pool, key_id, "hash2").await?;
    let keys = fetch_api_keys(&pool).await?;
    assert!(!keys.iter().any(|key| key.key_id == key_id));
    let auth = fetch_api_key_auth(&pool, key_id).await?;
    assert!(auth.is_some());
    let hash = fetch_api_key_hash(&pool, key_id).await?;
    assert_eq!(hash.as_deref(), Some("hash2"));
    let deleted = delete_api_key(&pool, key_id).await?;
    assert_eq!(deleted, 1);

    Ok(())
}

#[tokio::test]
async fn config_factory_reset_clears_auth_material_and_restores_defaults() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!(
                "skipping config_factory_reset_clears_auth_material_and_restores_defaults: {err}"
            );
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;

    run_migrations(&pool).await?;

    let app_id = Uuid::parse_str(APP_PROFILE_ID)?;
    let baseline_app = fetch_app_profile_row(&pool, app_id).await?;
    let baseline_fs = fetch_fs_policy_row(&pool, Uuid::parse_str(FS_POLICY_ID)?).await?;

    update_app_mode(&pool, app_id, "active").await?;
    update_app_auth_mode(&pool, app_id, "api_key").await?;
    upsert_secret(&pool, "reset-secret", b"payload", "tester").await?;
    insert_api_key(
        &pool,
        &NewApiKey {
            key_id: "reset-key",
            hash: "hash",
            label: Some("reset"),
            enabled: true,
            burst: Some(5),
            per_seconds: Some(60),
            expires_at: Some(Utc::now() + ChronoDuration::minutes(10)),
        },
    )
    .await?;

    factory_reset(&pool).await?;

    let app_row = fetch_app_profile_row(&pool, app_id).await?;
    let fs_row = fetch_fs_policy_row(&pool, Uuid::parse_str(FS_POLICY_ID)?).await?;
    assert_eq!(app_row.mode, baseline_app.mode);
    assert_eq!(app_row.auth_mode, "none");
    assert_eq!(app_row.http_port, baseline_app.http_port);
    assert_eq!(fs_row.library_root, baseline_fs.library_root);
    assert!(fetch_secret_by_name(&pool, "reset-secret").await?.is_none());
    assert!(fetch_api_keys(&pool).await?.is_empty());

    Ok(())
}

#[tokio::test]
async fn config_setup_token_and_api_key_helpers_track_state_transitions() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!(
                "skipping config_setup_token_and_api_key_helpers_track_state_transitions: {err}"
            );
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;

    run_migrations(&pool).await?;

    insert_setup_token(
        &pool,
        &NewSetupToken {
            token_hash: "expired-token",
            expires_at: Utc::now() - ChronoDuration::minutes(1),
            issued_by: "tester",
        },
    )
    .await?;
    cleanup_expired_setup_tokens(&pool).await?;
    insert_setup_token(
        &pool,
        &NewSetupToken {
            token_hash: "active-token",
            expires_at: Utc::now() + ChronoDuration::minutes(5),
            issued_by: "tester",
        },
    )
    .await?;

    cleanup_expired_setup_tokens(&pool).await?;
    let active = fetch_active_setup_token(&pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected active setup token"))?;
    assert_eq!(active.token_hash, "active-token");

    mark_setup_token_consumed(&pool, active.id).await?;
    assert!(fetch_active_setup_token(&pool).await?.is_none());

    insert_setup_token(
        &pool,
        &NewSetupToken {
            token_hash: "active-token-2",
            expires_at: Utc::now() + ChronoDuration::minutes(5),
            issued_by: "tester",
        },
    )
    .await?;
    invalidate_active_setup_tokens(&pool).await?;
    assert!(fetch_active_setup_token(&pool).await?.is_none());

    insert_api_key(
        &pool,
        &NewApiKey {
            key_id: "stateful-key",
            hash: "hash-a",
            label: Some("stateful"),
            enabled: true,
            burst: Some(8),
            per_seconds: Some(30),
            expires_at: Some(Utc::now() + ChronoDuration::minutes(10)),
        },
    )
    .await?;
    let auth = fetch_api_key_auth(&pool, "stateful-key")
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected api key auth row"))?;
    assert_eq!(auth.label, Some("stateful".to_string()));
    assert_eq!(auth.rate_limit_burst, Some(8));
    assert_eq!(auth.rate_limit_per_seconds, Some(30));

    update_api_key_label(&pool, "stateful-key", Some("renamed")).await?;
    update_api_key_enabled(&pool, "stateful-key", false).await?;
    update_api_key_rate_limit(&pool, "stateful-key", Some(3), Some(15)).await?;
    update_api_key_expires_at(
        &pool,
        "stateful-key",
        Some(Utc::now() + ChronoDuration::hours(1)),
    )
    .await?;
    update_api_key_hash(&pool, "stateful-key", "hash-b").await?;

    let auth = fetch_api_key_auth(&pool, "stateful-key")
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected updated api key auth row"))?;
    assert_eq!(auth.label, Some("renamed".to_string()));
    assert!(!auth.enabled);
    assert_eq!(auth.rate_limit_burst, Some(3));
    assert_eq!(auth.rate_limit_per_seconds, Some(15));
    assert_eq!(
        fetch_api_key_hash(&pool, "stateful-key").await?,
        Some("hash-b".to_string())
    );
    assert_eq!(delete_api_key(&pool, "stateful-key").await?, 1);
    assert!(fetch_api_key_auth(&pool, "stateful-key").await?.is_none());

    Ok(())
}

#[tokio::test]
async fn config_fs_and_tracker_helpers_round_trip_full_state() -> anyhow::Result<()> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping config_fs_and_tracker_helpers_round_trip_full_state: {err}");
            return Ok(());
        }
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(postgres.connection_string())
        .await?;

    run_migrations(&pool).await?;

    let engine_id = Uuid::parse_str(ENGINE_PROFILE_ID)?;
    let fs_id = Uuid::parse_str(FS_POLICY_ID)?;

    update_fs_string_field(&pool, fs_id, FsStringField::LibraryRoot, "/srv/library").await?;
    update_fs_string_field(&pool, fs_id, FsStringField::Par2, "verify").await?;
    update_fs_string_field(&pool, fs_id, FsStringField::MoveMode, "hardlink").await?;
    update_fs_boolean_field(&pool, fs_id, FsBooleanField::Extract, true).await?;
    update_fs_boolean_field(&pool, fs_id, FsBooleanField::Flatten, true).await?;
    update_fs_array_field(
        &pool,
        fs_id,
        FsArrayField::CleanupKeep,
        &["**/*.mkv".to_string(), "**/*.srt".to_string()],
    )
    .await?;
    update_fs_array_field(
        &pool,
        fs_id,
        FsArrayField::CleanupDrop,
        &["**/sample/**".to_string(), "**/*.nfo".to_string()],
    )
    .await?;
    update_fs_array_field(
        &pool,
        fs_id,
        FsArrayField::AllowPaths,
        &["/srv/library".to_string(), "/srv/downloads".to_string()],
    )
    .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::ChmodFile, Some("640"))
        .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::ChmodDir, Some("750"))
        .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::Owner, Some("media"))
        .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::Group, Some("media"))
        .await?;
    update_fs_optional_string_field(&pool, fs_id, FsOptionalStringField::Umask, Some("027"))
        .await?;

    set_engine_list_values(
        &pool,
        engine_id,
        "listen_interfaces",
        &["0.0.0.0:6999".to_string(), "[::]:6999".to_string()],
    )
    .await?;
    set_engine_list_values(
        &pool,
        engine_id,
        "dht_router_nodes",
        &["router.utorrent.com:6881".to_string()],
    )
    .await?;
    set_engine_ip_filter(
        &pool,
        engine_id,
        &IpFilterUpdate {
            blocklist_url: Some("https://blocklist.example"),
            etag: Some("etag-2"),
            last_updated_at: Some(Utc::now()),
            last_error: Some("timed out"),
            cidrs: &["192.168.0.0/16".to_string(), "10.0.0.0/8".to_string()],
        },
    )
    .await?;
    set_engine_alt_speed(
        &pool,
        engine_id,
        &AltSpeedUpdate {
            download_bps: Some(1500),
            upload_bps: Some(500),
            schedule_start_minutes: Some(30),
            schedule_end_minutes: Some(90),
            days: &["mon".to_string(), "fri".to_string()],
        },
    )
    .await?;
    set_tracker_config(
        &pool,
        engine_id,
        &TrackerConfigUpdate {
            user_agent: Some("RevaerTest/2"),
            announce_ip: Some("203.0.113.2"),
            listen_interface: Some("eth0"),
            request_timeout_ms: Some(5_000),
            announce: TrackerAnnouncePolicy {
                announce_to_all: true,
                replace_trackers: true,
            },
            proxy_host: Some("proxy.example"),
            proxy_port: Some(8443),
            proxy_kind: Some("https"),
            proxy_username_secret: Some("proxy-user"),
            proxy_password_secret: Some("proxy-pass"),
            proxy: TrackerProxyPolicy { proxy_peers: true },
            ssl_cert: Some("cert.pem"),
            ssl_private_key: Some("key.pem"),
            ssl_ca_cert: Some("ca.pem"),
            tls: TrackerTlsPolicy { verify: false },
            auth_username_secret: Some("auth-user"),
            auth_password_secret: Some("auth-pass"),
            auth_cookie_secret: Some("auth-cookie"),
            default_urls: &["https://tracker.example/announce".to_string()],
            extra_urls: &["https://extra.example/announce".to_string()],
        },
    )
    .await?;
    set_peer_classes(
        &pool,
        engine_id,
        &PeerClassesUpdate {
            class_ids: &[1, 2],
            labels: &["fast".to_string(), "slow".to_string()],
            download_priorities: &[7, 2],
            upload_priorities: &[6, 1],
            connection_limit_factors: &[150, 75],
            ignore_unchoke_slots: &[true, false],
            default_class_ids: &[1, 2],
        },
    )
    .await?;

    let fs_row = fetch_fs_policy_row(&pool, fs_id).await?;
    assert_eq!(fs_row.library_root, "/srv/library");
    assert_eq!(fs_row.par2, "verify");
    assert_eq!(fs_row.move_mode, "hardlink");
    assert!(fs_row.extract);
    assert!(fs_row.flatten);
    assert_eq!(fs_row.cleanup_keep.len(), 2);
    assert_eq!(fs_row.cleanup_drop.len(), 2);
    assert_eq!(fs_row.allow_paths.len(), 2);
    assert_eq!(fs_row.owner.as_deref(), Some("media"));
    assert_eq!(fs_row.group.as_deref(), Some("media"));
    assert_eq!(fs_row.umask.as_deref(), Some("027"));

    let engine_row = fetch_engine_profile_row(&pool, engine_id).await?;
    assert_eq!(engine_row.listen_interfaces.len(), 2);
    assert_eq!(
        engine_row.dht_router_nodes,
        vec!["router.utorrent.com:6881".to_string()]
    );
    assert_eq!(engine_row.ip_filter_cidrs.len(), 2);
    assert_eq!(
        engine_row.ip_filter_blocklist_url.as_deref(),
        Some("https://blocklist.example")
    );
    assert_eq!(
        engine_row.alt_speed_days,
        vec!["mon".to_string(), "fri".to_string()]
    );
    assert_eq!(
        engine_row.tracker_proxy_host.as_deref(),
        Some("proxy.example")
    );
    assert_eq!(engine_row.tracker_proxy_port, Some(8443));
    assert_eq!(engine_row.tracker_proxy_kind.as_deref(), Some("https"));
    assert_eq!(
        engine_row.tracker_auth_username_secret.as_deref(),
        Some("auth-user")
    );
    assert_eq!(engine_row.peer_class_ids, vec![1, 2]);
    assert_eq!(engine_row.peer_class_default_ids, vec![1, 2]);

    Ok(())
}
