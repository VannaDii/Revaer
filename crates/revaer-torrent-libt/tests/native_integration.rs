use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use revaer_events::Event;
use revaer_test_support::fixtures::docker_available;
use revaer_torrent_core::{
    AddTorrent, AddTorrentOptions, TorrentEngine, TorrentRateLimit, TorrentSource,
    model::PieceDeadline,
};
use revaer_torrent_libt::types::StorageMode;
use revaer_torrent_libt::{
    ChokingAlgorithm, EncryptionPolicy, EngineRuntimeConfig, Ipv6Mode, LibtorrentEngine,
    SeedChokingAlgorithm, TrackerProxyRuntime, TrackerProxyType, TrackerRuntimeConfig,
};
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use uuid::Uuid;

const MAGNET_URI: &str = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=demo";

fn base_runtime_config(download: &TempDir, resume: &TempDir) -> EngineRuntimeConfig {
    EngineRuntimeConfig {
        download_root: download.path().to_string_lossy().into_owned(),
        resume_dir: resume.path().to_string_lossy().into_owned(),
        storage_mode: StorageMode::Sparse,
        use_partfile: true.into(),
        disk_read_mode: None,
        disk_write_mode: None,
        verify_piece_hashes: true.into(),
        cache_size: None,
        cache_expiry: None,
        coalesce_reads: true.into(),
        coalesce_writes: true.into(),
        use_disk_cache_pool: true.into(),
        listen_interfaces: Vec::new(),
        ipv6_mode: Ipv6Mode::Disabled,
        enable_dht: true,
        dht_bootstrap_nodes: Vec::new(),
        dht_router_nodes: Vec::new(),
        enable_lsd: false.into(),
        enable_upnp: false.into(),
        enable_natpmp: false.into(),
        enable_pex: false.into(),
        outgoing_ports: None,
        peer_dscp: None,
        anonymous_mode: false.into(),
        force_proxy: false.into(),
        prefer_rc4: false.into(),
        allow_multiple_connections_per_ip: false.into(),
        enable_outgoing_utp: false.into(),
        enable_incoming_utp: false.into(),
        sequential_default: false,
        auto_managed: true.into(),
        auto_manage_prefer_seeds: false.into(),
        dont_count_slow_torrents: true.into(),
        listen_port: Some(68_81),
        max_active: Some(2),
        download_rate_limit: Some(128_000),
        upload_rate_limit: Some(64_000),
        seed_ratio_limit: None,
        seed_time_limit: None,
        alt_speed: None,
        stats_interval_ms: None,
        connections_limit: None,
        connections_limit_per_torrent: None,
        unchoke_slots: None,
        half_open_limit: None,
        choking_algorithm: ChokingAlgorithm::FixedSlots,
        seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
        strict_super_seeding: false.into(),
        optimistic_unchoke_slots: None,
        max_queued_disk_bytes: None,
        encryption: EncryptionPolicy::Prefer,
        tracker: TrackerRuntimeConfig::default(),
        ip_filter: None,
        peer_classes: Vec::new(),
        default_peer_classes: Vec::new(),
        super_seeding: false.into(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_alerts_and_rate_limits_smoke() -> Result<()> {
    if env::var("REVAER_NATIVE_IT").is_err() {
        return Ok(());
    }
    if !docker_available() {
        return Ok(());
    }

    let download = TempDir::new().context("temp download dir")?;
    let resume = TempDir::new().context("temp resume dir")?;

    let bus = revaer_events::EventBus::with_capacity(64);
    let engine = LibtorrentEngine::new(bus.clone()).context("engine init")?;

    let config = base_runtime_config(&download, &resume);
    engine
        .apply_runtime_config(config)
        .await
        .context("apply config")?;

    let torrent_id = Uuid::new_v4();
    let add = AddTorrent {
        id: torrent_id,
        source: TorrentSource::magnet(MAGNET_URI),
        options: AddTorrentOptions::default(),
    };
    engine.add_torrent(add).await.context("add torrent")?;

    engine
        .update_limits(
            None,
            TorrentRateLimit {
                download_bps: Some(64_000),
                upload_bps: Some(32_000),
            },
        )
        .await
        .context("apply global limits")?;

    // Apply a piece deadline and clear it to ensure the native binding works end-to-end.
    engine
        .set_piece_deadline(
            torrent_id,
            PieceDeadline {
                piece: 0,
                deadline_ms: Some(1_000),
            },
        )
        .await
        .context("set piece deadline")?;
    engine
        .set_piece_deadline(
            torrent_id,
            PieceDeadline {
                piece: 0,
                deadline_ms: None,
            },
        )
        .await
        .context("clear piece deadline")?;

    let mut stream = bus.subscribe(None);
    let mut saw_added = false;

    let window = Duration::from_secs(15);
    while !saw_added {
        match timeout(window, stream.next()).await {
            Ok(Some(Ok(envelope))) => match envelope.event {
                Event::TorrentAdded { torrent_id: id, .. } if id == torrent_id => {
                    saw_added = true;
                }
                _ => {}
            },
            Ok(Some(Err(_))) => {}
            _ => break,
        }
    }

    assert!(saw_added, "did not observe torrent added event");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_applies_proxy_auth_and_seed_limits() -> Result<()> {
    if env::var("REVAER_NATIVE_IT").is_err() {
        return Ok(());
    }
    if !docker_available() {
        return Ok(());
    }

    let download = TempDir::new().context("temp download dir")?;
    let resume = TempDir::new().context("temp resume dir")?;

    let bus = revaer_events::EventBus::with_capacity(16);
    let engine = LibtorrentEngine::new(bus).context("engine init")?;

    let mut config = base_runtime_config(&download, &resume);
    config.seed_ratio_limit = Some(1.5);
    config.seed_time_limit = Some(3_600);
    config.tracker.proxy = Some(TrackerProxyRuntime {
        host: "proxy.local".into(),
        port: 8080,
        username: Some("proxy-user".into()),
        password: Some("proxy-pass".into()),
        username_secret: None,
        password_secret: None,
        kind: TrackerProxyType::Http,
        proxy_peers: false,
    });

    engine
        .apply_runtime_config(config)
        .await
        .context("apply config")?;

    let snapshot = engine
        .inspect_settings()
        .await
        .context("inspect settings")?;
    assert_eq!(snapshot.proxy_username.as_deref(), Some("proxy-user"));
    assert_eq!(snapshot.proxy_password.as_deref(), Some("proxy-pass"));
    assert_eq!(snapshot.share_ratio_limit, Some(1_500));
    assert_eq!(snapshot.seed_time_limit, Some(3_600));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_applies_ipv6_mode_to_listen_interfaces() -> Result<()> {
    if env::var("REVAER_NATIVE_IT").is_err() {
        return Ok(());
    }
    if !docker_available() {
        return Ok(());
    }

    let download = TempDir::new().context("temp download dir")?;
    let resume = TempDir::new().context("temp resume dir")?;

    let bus = revaer_events::EventBus::with_capacity(16);
    let engine = LibtorrentEngine::new(bus).context("engine init")?;

    let mut config = base_runtime_config(&download, &resume);
    config.listen_interfaces.clear();
    config.listen_port = Some(6_881);
    config.ipv6_mode = Ipv6Mode::PreferV6;

    engine
        .apply_runtime_config(config)
        .await
        .context("apply config")?;

    let snapshot = engine
        .inspect_settings()
        .await
        .context("inspect settings")?;
    assert_eq!(snapshot.listen_interfaces, "[::]:6881,0.0.0.0:6881");
    Ok(())
}
