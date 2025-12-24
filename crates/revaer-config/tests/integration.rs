use revaer_config::{
    ApiKeyPatch, AppMode, ConfigService, SecretPatch, SettingsChangeset, SettingsFacade,
    engine_profile::MAX_RATE_LIMIT_BPS,
};
use revaer_data::config::{
    EngineProfileUpdate, NatToggleSet, PrivacyToggleSet, QueuePolicySet, SeedingToggleSet,
    StorageToggleSet, update_engine_profile,
};
use revaer_test_support::postgres::start_postgres;
use serde_json::json;
use tokio::time::Duration;

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

    let (snapshot, mut stream) = service.watch_settings(Duration::from_millis(50)).await?;
    let changes = SettingsChangeset {
        app_profile: Some(json!({ "mode": "active", "immutable_keys": [] })),
        engine_profile: Some(json!({
            "dht": !snapshot.engine_profile.dht,
            "listen_port": snapshot.engine_profile.listen_port,
            "sequential_default": snapshot.engine_profile.sequential_default,
            "resume_dir": snapshot.engine_profile.resume_dir,
            "download_root": snapshot.engine_profile.download_root,
            "tracker": {},
        })),
        fs_policy: Some(json!({
            "flatten": !snapshot.fs_policy.flatten,
            "allow_paths": [snapshot.fs_policy.library_root],
            "cleanup_keep": ["**/*.mkv"],
            "cleanup_drop": [],
            "move_mode": "copy"
        })),
        api_keys: vec![ApiKeyPatch::Upsert {
            key_id: "ci-key".to_string(),
            label: Some("ci".to_string()),
            enabled: Some(true),
            secret: Some("super-secret".to_string()),
            rate_limit: Some(json!({"burst": 10, "per_seconds": 60})),
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
    service
        .validate_setup_token(&issued.plaintext)
        .await
        .expect("setup token should validate before consumption");
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
        .expect("api key should authenticate");
    assert_eq!(auth.key_id, "ci-key");

    let updated = tokio::time::timeout(Duration::from_secs(10), stream.next())
        .await
        .expect("settings watcher should produce a snapshot")?;
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
    let alt_speed = json!({
        "download_bps": MAX_RATE_LIMIT_BPS + 1,
        "upload_bps": -10,
        "schedule": {
            "days": ["Wed", "mon", "Monday", ""],
            "start": "01:00",
            "end": "02:30"
        }
    });
    let dht_bootstrap_nodes = serde_json::to_value(&profile.dht_bootstrap_nodes)?;
    let dht_router_nodes = serde_json::to_value(&profile.dht_router_nodes)?;
    let listen_interfaces = serde_json::to_value(&profile.listen_interfaces)?;

    let update = EngineProfileUpdate {
        id: profile.id,
        implementation: &profile.implementation,
        listen_port: profile.listen_port,
        dht: profile.dht,
        encryption: &profile.encryption,
        max_active: profile.max_active,
        max_download_bps: profile.max_download_bps,
        max_upload_bps: profile.max_upload_bps,
        seed_ratio_limit: profile.seed_ratio_limit,
        seed_time_limit: profile.seed_time_limit,
        seeding: SeedingToggleSet::from_flags([
            profile.sequential_default,
            bool::from(profile.super_seeding),
            bool::from(profile.strict_super_seeding),
        ]),
        queue: QueuePolicySet::from_flags([
            bool::from(profile.auto_managed),
            bool::from(profile.auto_manage_prefer_seeds),
            bool::from(profile.dont_count_slow_torrents),
        ]),
        choking_algorithm: &profile.choking_algorithm,
        seed_choking_algorithm: &profile.seed_choking_algorithm,
        optimistic_unchoke_slots: profile.optimistic_unchoke_slots,
        max_queued_disk_bytes: profile.max_queued_disk_bytes,
        resume_dir: &profile.resume_dir,
        download_root: &profile.download_root,
        storage_mode: &profile.storage_mode,
        storage: StorageToggleSet::from_flags([
            bool::from(profile.use_partfile),
            bool::from(profile.coalesce_reads),
            bool::from(profile.coalesce_writes),
            bool::from(profile.use_disk_cache_pool),
        ]),
        disk_read_mode: profile.disk_read_mode.as_deref(),
        disk_write_mode: profile.disk_write_mode.as_deref(),
        verify_piece_hashes: bool::from(profile.verify_piece_hashes),
        cache_size: profile.cache_size,
        cache_expiry: profile.cache_expiry,
        tracker: &profile.tracker,
        nat: NatToggleSet::from_flags([
            bool::from(profile.enable_lsd),
            bool::from(profile.enable_upnp),
            bool::from(profile.enable_natpmp),
            bool::from(profile.enable_pex),
        ]),
        dht_bootstrap_nodes: &dht_bootstrap_nodes,
        dht_router_nodes: &dht_router_nodes,
        ip_filter: &profile.ip_filter,
        peer_classes: &profile.peer_classes,
        listen_interfaces: &listen_interfaces,
        ipv6_mode: &profile.ipv6_mode,
        privacy: PrivacyToggleSet::from_flags([
            bool::from(profile.anonymous_mode),
            bool::from(profile.force_proxy),
            bool::from(profile.prefer_rc4),
            bool::from(profile.allow_multiple_connections_per_ip),
            bool::from(profile.enable_outgoing_utp),
            bool::from(profile.enable_incoming_utp),
        ]),
        outgoing_port_min: profile.outgoing_port_min,
        outgoing_port_max: profile.outgoing_port_max,
        peer_dscp: profile.peer_dscp,
        connections_limit: profile.connections_limit,
        connections_limit_per_torrent: profile.connections_limit_per_torrent,
        unchoke_slots: profile.unchoke_slots,
        half_open_limit: profile.half_open_limit,
        alt_speed: &alt_speed,
        stats_interval_ms: profile
            .stats_interval_ms
            .and_then(|value| i32::try_from(value).ok()),
    };

    update_engine_profile(service.pool(), &update).await?;

    let refreshed = service.get_engine_profile().await?;
    assert_eq!(
        refreshed.alt_speed,
        json!({
            "download_bps": MAX_RATE_LIMIT_BPS,
            "schedule": {
                "days": ["mon", "wed"],
                "start": "01:00",
                "end": "02:30"
            }
        })
    );
    Ok(())
}
