//! Safe wrapper around the libtorrent worker and FFI bindings.

use anyhow::{Result, anyhow};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::command::EngineCommand;
use crate::store::FastResumeStore;
use crate::types::EngineRuntimeConfig;
use crate::worker;
use revaer_events::EventBus;
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentEngine, TorrentRateLimit,
    model::{
        PieceDeadline, TorrentAuthorRequest, TorrentAuthorResult, TorrentOptionsUpdate,
        TorrentTrackersUpdate, TorrentWebSeedsUpdate,
    },
};

const COMMAND_BUFFER: usize = 128;

/// Thin wrapper around the libtorrent bindings that also emits domain events.
#[derive(Clone)]
pub struct LibtorrentEngine {
    commands: mpsc::Sender<EngineCommand>,
}

impl LibtorrentEngine {
    /// Construct a new engine publisher hooked up to the shared event bus.
    ///
    /// # Errors
    ///
    /// Returns an error if the native libtorrent session cannot be initialized.
    pub fn new(events: EventBus) -> Result<Self> {
        Self::build(events, None)
    }

    /// Construct an engine with a configured fast-resume store.
    ///
    /// # Errors
    ///
    /// Returns an error if the native libtorrent session cannot be initialized.
    pub fn with_resume_store(events: EventBus, store: FastResumeStore) -> Result<Self> {
        Self::build(events, Some(store))
    }

    /// Apply the runtime configuration produced from the active engine profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration could not be enqueued for the background worker.
    pub async fn apply_runtime_config(&self, config: EngineRuntimeConfig) -> Result<()> {
        self.send_command(EngineCommand::ApplyConfig(Box::new(config)))
            .await
    }

    fn build(events: EventBus, store: Option<FastResumeStore>) -> Result<Self> {
        let session = crate::session::create_session()?;
        let (commands, rx) = mpsc::channel(COMMAND_BUFFER);
        if let Some(store_ref) = store.as_ref() {
            store_ref.ensure_initialized()?;
        }
        worker::spawn(events, rx, store, session);

        Ok(Self { commands })
    }

    async fn send_command(&self, command: EngineCommand) -> Result<()> {
        self.commands
            .send(command)
            .await
            .map_err(|err| anyhow!("failed to enqueue libtorrent command: {err}"))
    }
}

#[async_trait::async_trait]
impl TorrentEngine for LibtorrentEngine {
    async fn add_torrent(&self, request: AddTorrent) -> Result<()> {
        self.send_command(EngineCommand::Add(Box::new(request)))
            .await
    }

    async fn create_torrent(&self, request: TorrentAuthorRequest) -> Result<TorrentAuthorResult> {
        let (respond_to, rx) = oneshot::channel();
        self.send_command(EngineCommand::CreateTorrent {
            request,
            respond_to,
        })
        .await?;
        rx.await
            .map_err(|err| anyhow!("torrent authoring response dropped: {err}"))?
    }

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.send_command(EngineCommand::Remove { id, options })
            .await
    }

    async fn pause_torrent(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Pause { id }).await
    }

    async fn resume_torrent(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Resume { id }).await
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> Result<()> {
        self.send_command(EngineCommand::SetSequential { id, sequential })
            .await
    }

    async fn update_limits(&self, id: Option<Uuid>, limits: TorrentRateLimit) -> Result<()> {
        self.send_command(EngineCommand::UpdateLimits { id, limits })
            .await
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> Result<()> {
        self.send_command(EngineCommand::UpdateSelection { id, rules })
            .await
    }

    async fn update_options(&self, id: Uuid, options: TorrentOptionsUpdate) -> Result<()> {
        self.send_command(EngineCommand::UpdateOptions { id, options })
            .await
    }

    async fn update_trackers(&self, id: Uuid, trackers: TorrentTrackersUpdate) -> Result<()> {
        self.send_command(EngineCommand::UpdateTrackers { id, trackers })
            .await
    }

    async fn update_web_seeds(&self, id: Uuid, web_seeds: TorrentWebSeedsUpdate) -> Result<()> {
        self.send_command(EngineCommand::UpdateWebSeeds { id, web_seeds })
            .await
    }

    async fn reannounce(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Reannounce { id }).await
    }

    async fn move_torrent(&self, id: Uuid, download_dir: String) -> Result<()> {
        self.send_command(EngineCommand::MoveStorage { id, download_dir })
            .await
    }

    async fn recheck(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Recheck { id }).await
    }

    async fn set_piece_deadline(&self, id: Uuid, deadline: PieceDeadline) -> Result<()> {
        self.send_command(EngineCommand::SetPieceDeadline {
            id,
            piece: deadline.piece,
            deadline_ms: deadline.deadline_ms,
        })
        .await
    }

    async fn peers(&self, id: Uuid) -> Result<Vec<PeerSnapshot>> {
        let (respond_to, rx) = oneshot::channel();
        self.send_command(EngineCommand::QueryPeers { id, respond_to })
            .await?;
        rx.await
            .map_err(|err| anyhow!("peer query response dropped: {err}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::FastResumeStore;
    use crate::types::{
        ChokingAlgorithm, EncryptionPolicy, EngineRuntimeConfig, Ipv6Mode, SeedChokingAlgorithm,
        StorageMode, TrackerRuntimeConfig,
    };
    use revaer_torrent_core::{
        AddTorrentOptions, TorrentSource,
        model::{TorrentOptionsUpdate, TorrentTrackersUpdate, TorrentWebSeedsUpdate},
    };

    fn runtime_config_template(
        download_root: impl Into<String>,
        resume_dir: impl Into<String>,
    ) -> EngineRuntimeConfig {
        EngineRuntimeConfig {
            download_root: download_root.into(),
            resume_dir: resume_dir.into(),
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
            enable_dht: false,
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            outgoing_ports: None,
            peer_dscp: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            choking_algorithm: ChokingAlgorithm::FixedSlots,
            seed_choking_algorithm: SeedChokingAlgorithm::RoundRobin,
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            sequential_default: true,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: None,
            stats_interval_ms: None,
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
            peer_classes: Vec::new(),
            default_peer_classes: Vec::new(),
        }
    }

    #[tokio::test]
    async fn libtorrent_engine_accepts_command_flow() -> Result<()> {
        let events = EventBus::new();
        let engine = LibtorrentEngine::new(events)?;

        let mut runtime = runtime_config_template("/tmp/revaer-downloads", "/tmp/revaer-resume");
        runtime.enable_dht = true;
        runtime.sequential_default = false;
        runtime.listen_port = Some(6_881);
        runtime.max_active = Some(4);
        runtime.download_rate_limit = Some(1_000_000);
        runtime.upload_rate_limit = Some(500_000);
        engine.apply_runtime_config(runtime).await?;

        let torrent_id = Uuid::new_v4();
        let request = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
            options: AddTorrentOptions {
                name_hint: Some("demo".into()),
                ..AddTorrentOptions::default()
            },
        };

        engine.add_torrent(request.clone()).await?;
        engine.pause_torrent(torrent_id).await?;
        engine.resume_torrent(torrent_id).await?;
        engine.set_sequential(torrent_id, true).await?;
        engine
            .update_limits(
                Some(torrent_id),
                TorrentRateLimit {
                    download_bps: Some(256_000),
                    upload_bps: Some(128_000),
                },
            )
            .await?;
        engine
            .update_selection(torrent_id, FileSelectionUpdate::default())
            .await?;
        engine
            .update_trackers(
                torrent_id,
                TorrentTrackersUpdate {
                    trackers: vec!["http://tracker.example".into()],
                    replace: true,
                },
            )
            .await?;
        engine
            .update_web_seeds(
                torrent_id,
                TorrentWebSeedsUpdate {
                    web_seeds: vec!["http://seed.example/file".into()],
                    replace: true,
                },
            )
            .await?;
        engine
            .update_options(
                torrent_id,
                TorrentOptionsUpdate {
                    connections_limit: Some(4),
                    ..TorrentOptionsUpdate::default()
                },
            )
            .await?;
        engine.reannounce(torrent_id).await?;
        engine.recheck(torrent_id).await?;
        engine
            .remove_torrent(torrent_id, RemoveTorrent { with_data: false })
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn resume_store_is_initialized_when_provided() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let resume_dir = dir.path().join("resume");
        let store = FastResumeStore::new(&resume_dir);

        let events = EventBus::with_capacity(8);
        let engine = LibtorrentEngine::with_resume_store(events, store)?;
        engine
            .apply_runtime_config(runtime_config_template(
                "/tmp/revaer-downloads",
                resume_dir.display().to_string(),
            ))
            .await?;

        assert!(
            resume_dir.exists(),
            "fast-resume store should ensure directory exists"
        );
        Ok(())
    }
}
