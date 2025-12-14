//! Safe wrapper around the libtorrent worker and FFI bindings.

use anyhow::{Result, anyhow};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::command::EngineCommand;
use crate::store::FastResumeStore;
use crate::types::EngineRuntimeConfig;
use crate::worker;
use revaer_events::EventBus;
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentRateLimit,
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
        self.send_command(EngineCommand::Add(request)).await
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

    async fn reannounce(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Reannounce { id }).await
    }

    async fn recheck(&self, id: Uuid) -> Result<()> {
        self.send_command(EngineCommand::Recheck { id }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::FastResumeStore;
    use crate::types::{EncryptionPolicy, EngineRuntimeConfig, Ipv6Mode, TrackerRuntimeConfig};
    use revaer_torrent_core::{AddTorrentOptions, TorrentSource};

    #[tokio::test]
    async fn libtorrent_engine_accepts_command_flow() -> Result<()> {
        let events = EventBus::new();
        let engine = LibtorrentEngine::new(events)?;

        let runtime = EngineRuntimeConfig {
            download_root: "/tmp/revaer-downloads".into(),
            resume_dir: "/tmp/revaer-resume".into(),
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
            listen_port: Some(6_881),
            max_active: Some(4),
            download_rate_limit: Some(1_000_000),
            upload_rate_limit: Some(500_000),
            encryption: EncryptionPolicy::Prefer,
            tracker: TrackerRuntimeConfig::default(),
            ip_filter: None,
        };
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
            .apply_runtime_config(EngineRuntimeConfig {
                download_root: "/tmp/revaer-downloads".into(),
                resume_dir: resume_dir.display().to_string(),
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
                anonymous_mode: false.into(),
                force_proxy: false.into(),
                prefer_rc4: false.into(),
                allow_multiple_connections_per_ip: false.into(),
                enable_outgoing_utp: false.into(),
                enable_incoming_utp: false.into(),
                sequential_default: true,
                listen_port: None,
                max_active: None,
                download_rate_limit: None,
                upload_rate_limit: None,
                encryption: EncryptionPolicy::Prefer,
                tracker: TrackerRuntimeConfig::default(),
                ip_filter: None,
            })
            .await?;

        assert!(
            resume_dir.exists(),
            "fast-resume store should ensure directory exists"
        );
        Ok(())
    }
}
