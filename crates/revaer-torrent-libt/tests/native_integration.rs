use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use revaer_events::{Event, TorrentState};
use revaer_test_support::fixtures::docker_available;
use revaer_torrent_core::{
    AddTorrent, AddTorrentOptions, TorrentEngine, TorrentRateLimit, TorrentSource,
};
use revaer_torrent_libt::{
    EncryptionPolicy, EngineRuntimeConfig, Ipv6Mode, LibtorrentEngine, TrackerRuntimeConfig,
};
use tempfile::TempDir;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use uuid::Uuid;

const MAGNET_URI: &str = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=demo";

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

    let config = EngineRuntimeConfig {
        download_root: download.path().to_string_lossy().into_owned(),
        resume_dir: resume.path().to_string_lossy().into_owned(),
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
        listen_port: Some(68_81),
        max_active: Some(2),
        download_rate_limit: Some(128_000),
        upload_rate_limit: Some(64_000),
        seed_ratio_limit: None,
        seed_time_limit: None,
        alt_speed: None,
        connections_limit: None,
        connections_limit_per_torrent: None,
        unchoke_slots: None,
        half_open_limit: None,
        encryption: EncryptionPolicy::Prefer,
        tracker: TrackerRuntimeConfig::default(),
        ip_filter: None,
    };
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

    let mut stream = bus.subscribe(None);
    let mut saw_progress = false;
    let mut saw_state = false;

    let window = Duration::from_secs(15);
    while !(saw_progress && saw_state) {
        match timeout(window, stream.next()).await {
            Ok(Some(Ok(envelope))) => match envelope.event {
                Event::Progress { torrent_id: id, .. } if id == torrent_id => {
                    saw_progress = true;
                }
                Event::Completed { torrent_id: id, .. } if id == torrent_id => {
                    saw_progress = true;
                }
                Event::StateChanged {
                    torrent_id: id,
                    state,
                } if id == torrent_id
                    && matches!(state, TorrentState::Completed | TorrentState::Seeding) =>
                {
                    saw_state = true;
                }
                _ => {}
            },
            Ok(Some(Err(_))) => {}
            _ => break,
        }
    }

    assert!(saw_progress, "did not observe progress/complete event");
    assert!(saw_state, "did not observe completion/seeding state");
    Ok(())
}
