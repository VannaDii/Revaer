use super::*;

use crate::LabelKind;
use anyhow::{Result, anyhow};
use chrono::Duration as ChronoDuration;
use revaer_data::config::{NatToggleSet, PrivacyToggleSet, QueuePolicySet, StorageToggleSet};
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::str::FromStr;
use uuid::Uuid;

fn sample_label_row(kind: &str) -> LabelPolicyRow {
    LabelPolicyRow {
        kind: kind.to_string(),
        name: "movies".to_string(),
        download_dir: Some("/data/movies".to_string()),
        rate_limit_download_bps: Some(1024),
        rate_limit_upload_bps: Some(2048),
        queue_position: Some(2),
        auto_managed: Some(true),
        seed_ratio_limit: Some(1.5),
        seed_time_limit: Some(3600),
        cleanup_seed_ratio_limit: Some(2.5),
        cleanup_seed_time_limit: Some(1800),
        cleanup_remove_data: Some(false),
    }
}

fn sample_app_row() -> AppProfileRow {
    AppProfileRow {
        id: Uuid::new_v4(),
        instance_name: "Revaer".to_string(),
        mode: "setup".to_string(),
        auth_mode: "api_key".to_string(),
        version: 3,
        http_port: 8080,
        bind_addr: "127.0.0.1/32".to_string(),
        local_networks: vec!["127.0.0.0/8".to_string()],
        telemetry_level: Some("info".to_string()),
        telemetry_format: Some("json".to_string()),
        telemetry_otel_enabled: Some(true),
        telemetry_otel_service_name: Some("revaer".to_string()),
        telemetry_otel_endpoint: Some("http://otel.local".to_string()),
        immutable_keys: vec!["app_profile.mode".to_string()],
    }
}

fn sample_engine_row() -> EngineProfileRow {
    EngineProfileRow {
        id: Uuid::new_v4(),
        implementation: "stub".to_string(),
        listen_port: Some(6881),
        dht: true,
        encryption: "prefer_plaintext".to_string(),
        max_active: Some(12),
        max_download_bps: Some(10_000),
        max_upload_bps: Some(5_000),
        seed_ratio_limit: Some(2.0),
        seed_time_limit: Some(7200),
        queue: QueuePolicySet::from_flags([true, false, true]),
        seeding: SeedingToggleSet::from_flags([true, false, true]),
        choking_algorithm: "fixed".to_string(),
        seed_choking_algorithm: "round_robin".to_string(),
        optimistic_unchoke_slots: Some(4),
        max_queued_disk_bytes: Some(4096),
        resume_dir: "/tmp/resume".to_string(),
        download_root: "/tmp/downloads".to_string(),
        storage_mode: "sparse".to_string(),
        storage: StorageToggleSet::from_flags([true, true, false, true]),
        disk_read_mode: Some("sparse".to_string()),
        disk_write_mode: Some("normal".to_string()),
        verify_piece_hashes: true,
        cache_size: Some(256),
        cache_expiry: Some(120),
        tracker_user_agent: Some("Revaer/1.0".to_string()),
        tracker_announce_ip: Some("192.168.1.10".to_string()),
        tracker_listen_interface: Some("eth0".to_string()),
        tracker_request_timeout_ms: Some(3000),
        tracker_announce_to_all: Some(true),
        tracker_replace_trackers: Some(true),
        tracker_proxy_host: Some("proxy.local".to_string()),
        tracker_proxy_port: Some(70000),
        tracker_proxy_kind: Some("socks5".to_string()),
        tracker_proxy_username_secret: Some("proxy_user".to_string()),
        tracker_proxy_password_secret: Some("proxy_pass".to_string()),
        tracker_auth_username_secret: Some("auth_user".to_string()),
        tracker_auth_password_secret: Some("auth_pass".to_string()),
        tracker_auth_cookie_secret: Some("auth_cookie".to_string()),
        tracker_ssl_cert: Some("cert.pem".to_string()),
        tracker_ssl_private_key: Some("key.pem".to_string()),
        tracker_ssl_ca_cert: Some("ca.pem".to_string()),
        tracker_ssl_verify: Some(false),
        tracker_proxy_peers: Some(true),
        tracker_default_urls: vec!["udp://tracker.example".to_string()],
        tracker_extra_urls: vec!["udp://extra.example".to_string()],
        nat: NatToggleSet::from_flags([true, false, true, false]),
        dht_bootstrap_nodes: vec!["router.bittorrent.com:6881".to_string()],
        dht_router_nodes: vec!["router.utorrent.com:6881".to_string()],
        ip_filter_blocklist_url: Some("https://blocklist.local".to_string()),
        ip_filter_etag: Some("etag-1".to_string()),
        ip_filter_last_updated_at: Some(Utc::now()),
        ip_filter_last_error: Some("timeout".to_string()),
        ip_filter_cidrs: vec!["10.0.0.0/8".to_string()],
        peer_class_ids: vec![1, -1],
        peer_class_labels: vec!["fast".to_string(), "slow".to_string()],
        peer_class_download_priorities: vec![3, -2],
        peer_class_upload_priorities: vec![2, -1],
        peer_class_connection_limit_factors: vec![110, -5],
        peer_class_ignore_unchoke_slots: vec![true, false],
        peer_class_default_ids: vec![1, 300],
        listen_interfaces: vec!["0.0.0.0:6881".to_string()],
        ipv6_mode: "enabled".to_string(),
        privacy: PrivacyToggleSet::from_flags([true, true, false, false, true, false]),
        outgoing_port_min: Some(1000),
        outgoing_port_max: Some(2000),
        peer_dscp: Some(42),
        connections_limit: Some(500),
        connections_limit_per_torrent: Some(60),
        unchoke_slots: Some(4),
        half_open_limit: Some(50),
        alt_speed_download_bps: Some(500),
        alt_speed_upload_bps: Some(250),
        alt_speed_schedule_start_minutes: Some(60),
        alt_speed_schedule_end_minutes: Some(180),
        alt_speed_days: vec!["mon".to_string(), "Wed".to_string()],
        stats_interval_ms: Some(15000),
    }
}

fn sample_engine_profile() -> EngineProfile {
    map_engine_profile_row(sample_engine_row())
}

fn sample_fs_policy_row() -> FsPolicyRow {
    FsPolicyRow {
        id: Uuid::new_v4(),
        library_root: "/library".to_string(),
        extract: true,
        par2: "enabled".to_string(),
        flatten: false,
        move_mode: "copy".to_string(),
        cleanup_keep: vec!["**/*.mkv".to_string()],
        cleanup_drop: vec!["**/sample/**".to_string()],
        chmod_file: Some("644".to_string()),
        chmod_dir: Some("755".to_string()),
        owner: Some("media".to_string()),
        group: Some("media".to_string()),
        umask: Some("022".to_string()),
        allow_paths: vec!["/library".to_string()],
    }
}

#[test]
fn app_mode_parses_and_formats() -> anyhow::Result<()> {
    assert_eq!(AppMode::from_str("setup")?, AppMode::Setup);
    assert_eq!(AppMode::from_str("active")?, AppMode::Active);
    assert!(AppMode::from_str("invalid").is_err());
    assert_eq!(AppMode::Setup.as_str(), "setup");
    assert_eq!(AppMode::Active.as_str(), "active");
    Ok(())
}

#[test]
fn validate_port_accepts_valid_range() -> anyhow::Result<()> {
    validate_port(8080, "app_profile", "http_port")?;
    Ok(())
}

#[test]
fn validate_port_rejects_out_of_range() -> anyhow::Result<()> {
    let err = validate_port(0, "app_profile", "http_port")
        .err()
        .ok_or_else(|| anyhow!("expected invalid field error"))?;
    match err {
        ConfigError::InvalidField { reason, .. } => {
            assert_eq!(reason, "must be between 1 and 65535");
            Ok(())
        }
        _ => Err(anyhow!("unexpected config error variant")),
    }
}

#[test]
fn map_app_profile_row_maps_fields_and_labels() -> anyhow::Result<()> {
    let row = sample_app_row();
    let labels = vec![sample_label_row("category")];
    let profile = map_app_profile_row(row.clone(), labels)?;
    assert_eq!(profile.id, row.id);
    assert_eq!(profile.instance_name, "Revaer");
    assert_eq!(profile.mode, AppMode::Setup);
    assert_eq!(profile.auth_mode, AppAuthMode::ApiKey);
    assert_eq!(profile.http_port, 8080);
    assert_eq!(
        profile.bind_addr,
        IpAddr::from_str("127.0.0.1").expect("valid bind addr")
    );
    assert_eq!(profile.local_networks, vec!["127.0.0.0/8".to_string()]);
    assert_eq!(profile.telemetry.level.as_deref(), Some("info"));
    assert_eq!(profile.label_policies.len(), 1);
    assert_eq!(profile.label_policies[0].kind, LabelKind::Category);
    Ok(())
}

#[test]
fn map_app_profile_row_defaults_local_networks_when_entries_are_empty_or_invalid()
-> anyhow::Result<()> {
    let mut empty_networks = sample_app_row();
    empty_networks.local_networks.clear();
    let empty_profile = map_app_profile_row(empty_networks, Vec::new())?;
    assert_eq!(empty_profile.local_networks, default_local_networks());

    let mut invalid_networks = sample_app_row();
    invalid_networks.local_networks = vec!["not-a-cidr".to_string()];
    let invalid_profile = map_app_profile_row(invalid_networks, Vec::new())?;
    assert_eq!(invalid_profile.local_networks, default_local_networks());
    Ok(())
}

#[test]
fn map_label_policies_rejects_invalid_kind() -> anyhow::Result<()> {
    let err = map_label_policies(vec![sample_label_row("invalid")])
        .err()
        .ok_or_else(|| anyhow!("expected invalid label kind error"))?;
    assert!(matches!(err, ConfigError::InvalidLabelKind { .. }));
    Ok(())
}

#[test]
fn parse_tracker_proxy_kind_handles_known_values() {
    assert_eq!(parse_tracker_proxy_kind(None), TrackerProxyType::Http);
    assert_eq!(
        parse_tracker_proxy_kind(Some("HTTPS")),
        TrackerProxyType::Https
    );
    assert_eq!(
        parse_tracker_proxy_kind(Some("socks5")),
        TrackerProxyType::Socks5
    );
    assert_eq!(
        parse_tracker_proxy_kind(Some("custom")),
        TrackerProxyType::Http
    );
}

#[test]
fn parse_weekday_label_accepts_aliases() {
    assert_eq!(parse_weekday_label("Mon"), Some(Weekday::Mon));
    assert_eq!(parse_weekday_label("tues"), Some(Weekday::Tue));
    assert_eq!(parse_weekday_label("WEDNESDAY"), Some(Weekday::Wed));
    assert_eq!(parse_weekday_label("thurs"), Some(Weekday::Thu));
    assert_eq!(parse_weekday_label("fri"), Some(Weekday::Fri));
    assert_eq!(parse_weekday_label("SATURDAY"), Some(Weekday::Sat));
    assert_eq!(parse_weekday_label("sun"), Some(Weekday::Sun));
    assert_eq!(parse_weekday_label("noday"), None);
}

#[test]
fn map_tracker_config_builds_proxy_and_auth() -> anyhow::Result<()> {
    let row = sample_engine_row();
    let tracker = map_tracker_config(&row);
    let proxy = tracker
        .proxy
        .ok_or_else(|| anyhow!("expected tracker proxy"))?;
    assert_eq!(proxy.host, "proxy.local");
    assert_eq!(proxy.port, 0);
    assert_eq!(proxy.kind, TrackerProxyType::Socks5);
    assert_eq!(proxy.username_secret.as_deref(), Some("proxy_user"));
    assert_eq!(proxy.password_secret.as_deref(), Some("proxy_pass"));
    assert!(proxy.proxy_peers);
    let auth = tracker
        .auth
        .ok_or_else(|| anyhow!("expected tracker auth"))?;
    assert_eq!(auth.username_secret.as_deref(), Some("auth_user"));
    assert_eq!(auth.password_secret.as_deref(), Some("auth_pass"));
    assert_eq!(auth.cookie_secret.as_deref(), Some("auth_cookie"));
    Ok(())
}

#[test]
fn map_tracker_config_omits_partial_proxy_and_missing_auth() {
    let mut row = sample_engine_row();
    row.tracker_proxy_port = None;
    row.tracker_auth_username_secret = None;
    row.tracker_auth_password_secret = None;
    row.tracker_auth_cookie_secret = None;

    let tracker = map_tracker_config(&row);
    assert!(tracker.proxy.is_none());
    assert!(tracker.auth.is_none());
}

#[test]
fn map_alt_speed_config_requires_schedule_and_days() -> anyhow::Result<()> {
    let mut row = sample_engine_row();
    row.alt_speed_days = vec!["invalid".to_string()];
    let alt = map_alt_speed_config(&row);
    assert!(alt.schedule.is_none());

    row.alt_speed_days = vec!["mon".to_string(), "fri".to_string()];
    let alt = map_alt_speed_config(&row);
    let schedule = alt
        .schedule
        .ok_or_else(|| anyhow!("expected alt speed schedule"))?;
    assert_eq!(schedule.start_minutes, 60);
    assert_eq!(schedule.end_minutes, 180);
    assert_eq!(schedule.days.len(), 2);
    Ok(())
}

#[test]
fn map_alt_speed_config_clamps_negative_minutes_before_building_schedule() -> anyhow::Result<()> {
    let mut row = sample_engine_row();
    row.alt_speed_days = vec!["mon".to_string()];
    row.alt_speed_schedule_start_minutes = Some(-5);
    row.alt_speed_schedule_end_minutes = Some(30);

    let alt = map_alt_speed_config(&row);
    let schedule = alt
        .schedule
        .ok_or_else(|| anyhow!("expected clamped alt-speed schedule"))?;
    assert_eq!(schedule.days, vec![Weekday::Mon]);
    assert_eq!(schedule.start_minutes, 0);
    assert_eq!(schedule.end_minutes, 30);
    Ok(())
}

#[test]
fn map_peer_classes_config_truncates_and_defaults() {
    let row = sample_engine_row();
    let classes = map_peer_classes_config(&row);
    assert_eq!(classes.classes.len(), 2);
    assert_eq!(classes.classes[0].id, 1);
    assert_eq!(classes.classes[0].download_priority, 3);
    assert_eq!(classes.classes[0].upload_priority, 2);
    assert_eq!(classes.classes[0].connection_limit_factor, 110);
    assert_eq!(classes.classes[1].id, 0);
    assert_eq!(classes.classes[1].download_priority, 1);
    assert_eq!(classes.classes[1].upload_priority, 1);
    assert_eq!(classes.classes[1].connection_limit_factor, 100);
    assert_eq!(classes.default, vec![1]);
}

#[test]
fn map_engine_profile_row_maps_nested_config() {
    let row = sample_engine_row();
    let profile = map_engine_profile_row(row.clone());
    assert_eq!(profile.id, row.id);
    assert_eq!(profile.listen_port, row.listen_port);
    assert!(profile.sequential_default);
    assert!(profile.dont_count_slow_torrents.is_enabled());
    assert_eq!(profile.tracker.default, row.tracker_default_urls);
    assert_eq!(profile.alt_speed.download_bps, Some(500));
    assert_eq!(profile.ip_filter.cidrs, row.ip_filter_cidrs);
}

#[test]
fn map_fs_policy_row_maps_fields() {
    let row = sample_fs_policy_row();
    let policy = map_fs_policy_row(row.clone());
    assert_eq!(policy.id, row.id);
    assert_eq!(policy.library_root, row.library_root);
    assert!(policy.extract);
    assert_eq!(policy.allow_paths, row.allow_paths);
}

#[test]
fn generate_token_produces_alphanumeric_length() {
    let token = generate_token(32);
    assert_eq!(token.len(), 32);
    assert!(token.chars().all(|ch| ch.is_ascii_alphanumeric()));
}

#[test]
fn hash_and_verify_secret_round_trip() -> anyhow::Result<()> {
    let hash = hash_secret("super-secret")?;
    assert!(verify_secret(&hash, "super-secret")?);
    assert!(!verify_secret(&hash, "incorrect")?);
    Ok(())
}

#[test]
fn verify_secret_rejects_invalid_hash() -> anyhow::Result<()> {
    let err = verify_secret("not-a-hash", "secret")
        .err()
        .ok_or_else(|| anyhow!("expected invalid hash error"))?;
    assert!(matches!(err, ConfigError::StoredHashInvalid { .. }));
    Ok(())
}

#[tokio::test]
async fn validate_directory_path_rejects_missing_and_non_dir() -> Result<()> {
    let root = std::env::temp_dir().join(format!("revaer-config-{}", Uuid::new_v4()));
    let missing = root.join("missing");
    let missing_path = missing.to_string_lossy().to_string();
    let err = validate_directory_path("app_profile", "download_root", &missing_path)
        .await
        .err()
        .ok_or_else(|| anyhow!("expected missing path error"))?;
    assert!(matches!(err, ConfigError::InvalidField { .. }));

    fs::create_dir_all(&root)?;
    let file_path = root.join("file.txt");
    fs::write(&file_path, "data")?;
    let file_path = file_path.to_string_lossy().to_string();
    let err = validate_directory_path("app_profile", "download_root", &file_path)
        .await
        .err()
        .ok_or_else(|| anyhow!("expected non-directory error"))?;
    assert!(matches!(err, ConfigError::InvalidField { .. }));

    fs::remove_dir_all(&root)?;
    Ok(())
}

#[tokio::test]
async fn validate_allow_paths_rejects_missing_entries() -> Result<()> {
    let root = std::env::temp_dir().join(format!("revaer-config-{}", Uuid::new_v4()));
    let missing = root.join("missing");
    let err = validate_allow_paths(&[missing.to_string_lossy().to_string()])
        .await
        .err()
        .ok_or_else(|| anyhow!("expected allow path error"))?;
    assert!(matches!(err, ConfigError::InvalidField { .. }));
    Ok(())
}

#[test]
fn ensure_engine_profile_mutable_rejects_immutable_changes() {
    let current = sample_engine_profile();
    let mut update = current.clone();
    update.listen_port = Some(7000);

    let mut immutables = HashSet::new();
    immutables.insert("engine_profile.listen_port".to_string());

    let err = ensure_engine_profile_mutable(&current, &update, &immutables)
        .expect_err("expected immutable error");
    assert!(matches!(err, ConfigError::ImmutableField { .. }));
}

#[test]
fn normalize_engine_profile_for_storage_clamps_listen_port() {
    let mut profile = sample_engine_profile();
    profile.listen_port = Some(70_000);
    let normalized = normalize_engine_profile_for_storage(&profile);
    assert!(normalized.listen_port.is_none());
}

#[test]
fn validate_api_key_expiry_rejects_excess_ttl() {
    let expires_at = Utc::now() + ChronoDuration::days(API_KEY_TTL_DAYS + 1);
    let err = validate_api_key_expiry(Some(expires_at)).expect_err("expected expiry violation");
    assert!(matches!(err, ConfigError::InvalidField { .. }));
}

#[test]
fn validate_db_session_config_rejects_invalid_values() {
    let empty_key = validate_db_session_config(&DbSessionConfig::new("", "secret"))
        .expect_err("empty key id should be rejected");
    assert!(matches!(
        empty_key,
        ConfigError::InvalidField { field, reason, .. }
        if field == "secret_key_id" && reason == "empty"
    ));

    let whitespace_secret = validate_db_session_config(&DbSessionConfig::new("key-id", " secret "))
        .expect_err("whitespace should be rejected");
    assert!(matches!(
        whitespace_secret,
        ConfigError::InvalidField { field, reason, .. }
        if field == "secret_key" && reason == "whitespace"
    ));

    let too_long_key = "k".repeat(DbSessionConfig::SECRET_KEY_ID_MAX_LEN + 1);
    let too_long = validate_db_session_config(&DbSessionConfig::new(&too_long_key, "secret"))
        .expect_err("overlong key id should be rejected");
    assert!(matches!(
        too_long,
        ConfigError::InvalidField { field, reason, .. }
        if field == "secret_key_id" && reason == "too_long"
    ));
}
