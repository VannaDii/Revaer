#![allow(clippy::redundant_pub_crate)]

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use revaer_config::{EngineProfile, FsPolicy};
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_fsops::{FsOpsRequest, FsOpsService};
use revaer_runtime::RuntimeStore;
use revaer_telemetry::Metrics;
use revaer_torrent_core::{
    AddTorrent, FilePriority, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentFile,
    TorrentInspector, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentStatus,
    TorrentWorkflow,
};
use tokio::{sync::RwLock, task::JoinHandle};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Upper bound for rate limits before emitting guard-rail warnings (≈5 Gbps).
const RATE_LIMIT_GUARD_BPS: u64 = 5_000_000_000;

#[async_trait]
pub(crate) trait EngineConfigurator: Send + Sync {
    async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()>;
}

#[cfg(feature = "libtorrent")]
use revaer_torrent_libt::{EncryptionPolicy, EngineRuntimeConfig, LibtorrentEngine};

#[cfg(feature = "libtorrent")]
#[async_trait]
impl EngineConfigurator for LibtorrentEngine {
    async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()> {
        info!(
            implementation = %profile.implementation,
            listen_port = ?profile.listen_port,
            "applying engine profile update"
        );
        let config = runtime_config_from_profile(profile);
        self.apply_runtime_config(config).await
    }
}

#[cfg(feature = "libtorrent")]
fn runtime_config_from_profile(profile: &EngineProfile) -> EngineRuntimeConfig {
    EngineRuntimeConfig {
        download_root: profile.download_root.clone(),
        resume_dir: profile.resume_dir.clone(),
        enable_dht: profile.dht,
        sequential_default: profile.sequential_default,
        listen_port: profile.listen_port,
        max_active: profile.max_active,
        download_rate_limit: profile.max_download_bps,
        upload_rate_limit: profile.max_upload_bps,
        encryption: map_encryption_policy(&profile.encryption),
    }
}

#[cfg(feature = "libtorrent")]
fn map_encryption_policy(value: &str) -> EncryptionPolicy {
    match value.to_ascii_lowercase().as_str() {
        "require" | "required" => EncryptionPolicy::Require,
        "disable" | "disabled" => EncryptionPolicy::Disable,
        _ => EncryptionPolicy::Prefer,
    }
}

/// Coordinates torrent engine lifecycle with filesystem post-processing via the shared event bus.
#[cfg(any(feature = "libtorrent", test))]
pub(crate) struct TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    engine: Arc<E>,
    fsops: FsOpsService,
    events: EventBus,
    fs_policy: Arc<RwLock<FsPolicy>>,
    engine_profile: Arc<RwLock<EngineProfile>>,
    catalog: Arc<TorrentCatalog>,
    runtime: Option<RuntimeStore>,
}

#[cfg(any(feature = "libtorrent", test))]
impl<E> TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    /// Construct a new orchestrator with shared dependencies.
    #[must_use]
    pub(crate) fn new(
        engine: Arc<E>,
        fsops: FsOpsService,
        events: EventBus,
        fs_policy: FsPolicy,
        engine_profile: EngineProfile,
        runtime: Option<RuntimeStore>,
    ) -> Self {
        Self {
            engine,
            fsops,
            events,
            fs_policy: Arc::new(RwLock::new(fs_policy)),
            engine_profile: Arc::new(RwLock::new(engine_profile)),
            catalog: Arc::new(TorrentCatalog::new()),
            runtime,
        }
    }

    /// Delegate torrent admission to the engine.
    pub(crate) async fn add_torrent(&self, request: AddTorrent) -> Result<()> {
        self.engine.add_torrent(request).await
    }

    /// Apply the filesystem policy to a completed torrent.
    pub(crate) async fn apply_fsops(&self, torrent_id: Uuid) -> Result<()> {
        let policy = self.fs_policy.read().await.clone();
        let snapshot = self
            .catalog
            .get(torrent_id)
            .await
            .ok_or_else(|| anyhow!("torrent status unavailable for fsops application"))?;
        let source = snapshot
            .library_path
            .as_deref()
            .ok_or_else(|| anyhow!("library path missing for torrent {torrent_id}"))?;
        let source_path = PathBuf::from(source);
        self.fsops.apply(FsOpsRequest {
            torrent_id,
            source_path: &source_path,
            policy: &policy,
        })
    }

    async fn handle_event(&self, event: &Event) -> Result<()> {
        self.catalog.observe(event).await;
        self.persist_runtime(event).await;
        if let Event::Completed { torrent_id, .. } = event {
            self.apply_fsops(*torrent_id).await?;
        }
        Ok(())
    }

    async fn persist_runtime(&self, event: &Event) {
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let Some(torrent_id) = event_torrent_id(event) else {
            return;
        };

        match event {
            Event::TorrentRemoved { .. } => {
                if let Err(err) = runtime.remove_torrent(torrent_id).await {
                    warn!(
                        error = %err,
                        torrent_id = %torrent_id,
                        "failed to remove torrent from runtime store"
                    );
                }
            }
            _ => {
                if let Some(status) = self.catalog.get(torrent_id).await
                    && let Err(err) = runtime.upsert_status(&status).await
                {
                    warn!(
                        error = %err,
                        torrent_id = %torrent_id,
                        "failed to persist torrent status"
                    );
                }
            }
        }
    }

    /// Spawn a background task that reacts to completion events and triggers filesystem processing.
    pub(crate) fn spawn_post_processing(self: &Arc<Self>) -> JoinHandle<()> {
        let orchestrator = Arc::clone(self);
        tokio::spawn(async move {
            let mut stream = orchestrator.events.subscribe(None);
            while let Some(envelope) = stream.next().await {
                if let Err(err) = orchestrator.handle_event(&envelope.event).await {
                    error!(
                        error = %err,
                        "failed to apply filesystem policy after torrent completion"
                    );
                }
            }
        })
    }

    /// Update the active filesystem policy used for post-processing.
    pub(crate) async fn update_fs_policy(&self, policy: FsPolicy) {
        let mut guard = self.fs_policy.write().await;
        *guard = policy;
    }

    /// Update the engine profile and propagate changes to the underlying engine.
    pub(crate) async fn update_engine_profile(&self, profile: EngineProfile) -> Result<()> {
        {
            let mut guard = self.engine_profile.write().await;
            *guard = profile.clone();
        }
        let download_limit = profile
            .max_download_bps
            .and_then(|limit| u64::try_from(limit).ok());
        let upload_limit = profile
            .max_upload_bps
            .and_then(|limit| u64::try_from(limit).ok());

        info!(
            implementation = %profile.implementation,
            listen_port = ?profile.listen_port,
            max_active = ?profile.max_active,
            download_bps = ?download_limit,
            upload_bps = ?upload_limit,
            "applying engine profile update"
        );

        log_rate_guardrail("download", download_limit);
        log_rate_guardrail("upload", upload_limit);

        self.engine.apply_engine_profile(&profile).await?;
        self.engine
            .update_limits(
                None,
                TorrentRateLimit {
                    download_bps: download_limit,
                    upload_bps: upload_limit,
                },
            )
            .await?;
        Ok(())
    }

    /// Remove the torrent from the engine.
    pub(crate) async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.engine.remove_torrent(id, options).await
    }
}

fn log_rate_guardrail(direction: &str, limit: Option<u64>) {
    if let Some(value) = limit {
        log_rate_value(direction, value);
    } else {
        warn!(
            direction = direction,
            "global {direction} rate limit disabled; running without throttling"
        );
    }
}

fn log_rate_value(direction: &str, value: u64) {
    if value == 0 {
        warn!(
            direction = direction,
            "global {direction} rate limit set to 0 bps; transfers will halt"
        );
        return;
    }

    if value >= RATE_LIMIT_GUARD_BPS {
        warn!(
            direction = direction,
            current_bps = value,
            guard_bps = RATE_LIMIT_GUARD_BPS,
            "global {direction} rate limit exceeds guard rail"
        );
        return;
    }

    info!(
        direction = direction,
        current_bps = value,
        "applied global {direction} rate limit"
    );
}

const fn event_torrent_id(event: &Event) -> Option<Uuid> {
    match event {
        Event::TorrentAdded { torrent_id, .. }
        | Event::FilesDiscovered { torrent_id, .. }
        | Event::Progress { torrent_id, .. }
        | Event::StateChanged { torrent_id, .. }
        | Event::Completed { torrent_id, .. }
        | Event::TorrentRemoved { torrent_id }
        | Event::FsopsStarted { torrent_id }
        | Event::FsopsProgress { torrent_id, .. }
        | Event::FsopsCompleted { torrent_id }
        | Event::FsopsFailed { torrent_id, .. }
        | Event::SelectionReconciled { torrent_id, .. } => Some(*torrent_id),
        Event::SettingsChanged { .. } | Event::HealthChanged { .. } => None,
    }
}

#[cfg(feature = "libtorrent")]
pub(crate) async fn spawn_libtorrent_orchestrator(
    events: &EventBus,
    metrics: Metrics,
    fs_policy: FsPolicy,
    engine_profile: EngineProfile,
    runtime: Option<RuntimeStore>,
) -> Result<(
    Arc<LibtorrentEngine>,
    Arc<TorrentOrchestrator<LibtorrentEngine>>,
    JoinHandle<()>,
)> {
    let engine = Arc::new(LibtorrentEngine::new(events.clone())?);
    engine.apply_engine_profile(&engine_profile).await?;
    let fsops = runtime.clone().map_or_else(
        || FsOpsService::new(events.clone(), metrics.clone()),
        |store| FsOpsService::new(events.clone(), metrics.clone()).with_runtime(store),
    );
    let orchestrator = Arc::new(TorrentOrchestrator::new(
        Arc::clone(&engine),
        fsops,
        events.clone(),
        fs_policy,
        engine_profile,
        runtime.clone(),
    ));
    if let Some(store) = runtime {
        match store.load_statuses().await {
            Ok(statuses) => orchestrator.catalog.seed(statuses).await,
            Err(err) => warn!(
                error = %err,
                "failed to hydrate torrent catalog from runtime store"
            ),
        }
    }
    let worker = orchestrator.spawn_post_processing();
    Ok((engine, orchestrator, worker))
}

#[cfg(any(feature = "libtorrent", test))]
#[async_trait]
impl<E> TorrentWorkflow for TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
        Self::add_torrent(self, request).await
    }

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()> {
        Self::remove_torrent(self, id, options).await
    }

    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        self.engine.pause_torrent(id).await
    }

    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        self.engine.resume_torrent(id).await
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        self.engine.set_sequential(id, sequential).await
    }

    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        self.engine.update_limits(id, limits).await
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        self.engine.update_selection(id, rules).await
    }

    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        self.engine.reannounce(id).await
    }

    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        self.engine.recheck(id).await
    }
}

#[cfg(any(feature = "libtorrent", test))]
#[async_trait]
impl<E> TorrentInspector for TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
        Ok(self.catalog.list().await)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
        Ok(self.catalog.get(id).await)
    }
}

#[derive(Default)]
struct TorrentCatalog {
    entries: RwLock<HashMap<Uuid, TorrentStatus>>,
}

impl TorrentCatalog {
    fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    async fn seed(&self, statuses: Vec<TorrentStatus>) {
        let mut entries = self.entries.write().await;
        entries.clear();
        for status in statuses {
            entries.insert(status.id, status);
        }
    }

    async fn observe(&self, event: &Event) {
        if matches!(event, Event::FilesDiscovered { files, .. } if files.is_empty()) {
            return;
        }

        let mut entries = self.entries.write().await;
        Self::apply_event(&mut entries, event);
    }

    async fn list(&self) -> Vec<TorrentStatus> {
        let mut values: Vec<_> = {
            let entries = self.entries.read().await;
            entries.values().cloned().collect()
        };
        values.sort_by(Self::compare_status);
        values
    }

    async fn get(&self, id: Uuid) -> Option<TorrentStatus> {
        self.entries.read().await.get(&id).cloned()
    }

    fn blank_status(id: Uuid) -> TorrentStatus {
        let now = Utc::now();
        TorrentStatus {
            id,
            name: None,
            state: TorrentState::Queued,
            progress: TorrentProgress::default(),
            rates: TorrentRates::default(),
            files: None,
            library_path: None,
            download_dir: None,
            sequential: false,
            added_at: now,
            completed_at: None,
            last_updated: now,
        }
    }

    fn compare_status(a: &TorrentStatus, b: &TorrentStatus) -> Ordering {
        match (a.name.as_deref(), b.name.as_deref()) {
            (Some(a_name), Some(b_name)) => {
                let ordering = a_name.cmp(b_name);
                if ordering == Ordering::Equal {
                    a.id.cmp(&b.id)
                } else {
                    ordering
                }
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        }
    }

    fn apply_event(entries: &mut HashMap<Uuid, TorrentStatus>, event: &Event) {
        match event {
            Event::TorrentAdded { torrent_id, name } => {
                Self::record_torrent_added(entries, *torrent_id, name);
            }
            Event::FilesDiscovered { torrent_id, files } => {
                Self::record_files_discovered(entries, *torrent_id, files);
            }
            Event::Progress {
                torrent_id,
                bytes_downloaded,
                bytes_total,
            } => {
                Self::record_progress(entries, *torrent_id, *bytes_downloaded, *bytes_total);
            }
            Event::StateChanged { torrent_id, state } => {
                Self::record_state_change(entries, *torrent_id, state);
            }
            Event::Completed {
                torrent_id,
                library_path,
            } => {
                Self::record_completion(entries, *torrent_id, library_path);
            }
            Event::TorrentRemoved { torrent_id } => {
                entries.remove(torrent_id);
            }
            Event::FsopsFailed {
                torrent_id,
                message,
            } => {
                Self::record_fsops_failure(entries, *torrent_id, message);
            }
            Event::FsopsStarted { torrent_id }
            | Event::FsopsCompleted { torrent_id }
            | Event::FsopsProgress { torrent_id, .. } => {
                Self::touch_entry(entries, *torrent_id);
            }
            Event::SettingsChanged { .. }
            | Event::HealthChanged { .. }
            | Event::SelectionReconciled { .. } => {}
        }
    }

    fn record_torrent_added(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        name: &str,
    ) {
        let now = Utc::now();
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.name = Some(name.to_owned());
        entry.state = TorrentState::Queued;
        entry.progress = TorrentProgress::default();
        entry.library_path = None;
        entry.rates = TorrentRates::default();
        entry.added_at = now;
        entry.last_updated = now;
    }

    fn record_files_discovered(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        files: &[DiscoveredFile],
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.files = Some(Self::map_discovered_files(files));
        entry.last_updated = Utc::now();
    }

    fn record_progress(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        bytes_downloaded: u64,
        bytes_total: u64,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.progress.bytes_downloaded = bytes_downloaded;
        entry.progress.bytes_total = bytes_total;
        entry.progress.eta_seconds = None;
        entry.rates.download_bps = 0;
        entry.rates.upload_bps = 0;
        #[allow(clippy::cast_precision_loss)]
        let ratio = if bytes_total == 0 {
            0.0
        } else {
            (bytes_downloaded as f64) / (bytes_total as f64)
        };
        entry.rates.ratio = ratio;
        entry.last_updated = Utc::now();
    }

    fn record_state_change(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        state: &TorrentState,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.state = state.clone();
        entry.last_updated = Utc::now();
    }

    fn record_completion(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        library_path: &str,
    ) {
        let now = Utc::now();
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.state = TorrentState::Completed;
        entry.library_path = Some(library_path.to_owned());
        entry.completed_at = Some(now);
        entry.last_updated = now;
    }

    fn record_fsops_failure(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        message: &str,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.state = TorrentState::Failed {
            message: message.to_owned(),
        };
        entry.last_updated = Utc::now();
    }

    fn touch_entry(entries: &mut HashMap<Uuid, TorrentStatus>, torrent_id: Uuid) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.last_updated = Utc::now();
    }

    fn ensure_entry(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
    ) -> &mut TorrentStatus {
        entries
            .entry(torrent_id)
            .or_insert_with(|| Self::blank_status(torrent_id))
    }

    fn map_discovered_files(files: &[DiscoveredFile]) -> Vec<TorrentFile> {
        files
            .iter()
            .enumerate()
            .map(|(index, file)| {
                let index_u32 = u32::try_from(index).unwrap_or(u32::MAX);
                TorrentFile {
                    index: index_u32,
                    path: file.path.clone(),
                    size_bytes: file.size_bytes,
                    bytes_completed: 0,
                    priority: FilePriority::Normal,
                    selected: true,
                }
            })
            .collect()
    }
}

#[cfg(all(test, feature = "libtorrent"))]
mod tests {
    use super::*;
    use anyhow::{Context, bail};
    use revaer_config::ConfigService;
    use revaer_events::EventBus;
    use revaer_torrent_core::{AddTorrent, AddTorrentOptions, TorrentSource};
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;
    use testcontainers::core::{ContainerPort, WaitFor};
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{GenericImage, ImageExt};
    use tokio::task::yield_now;
    use tokio::time::{Duration, sleep, timeout};

    fn sample_fs_policy(library_root: &str) -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: library_root.to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!([]),
            cleanup_drop: json!([]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([]),
        }
    }

    fn sample_engine_profile() -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: "libtorrent".to_string(),
            listen_port: Some(6_881),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/tmp/resume".to_string(),
            download_root: "/downloads".to_string(),
            tracker: json!([]),
        }
    }

    #[tokio::test]
    async fn orchestrator_persists_runtime_state() -> Result<()> {
        let base_image = GenericImage::new("postgres", "14-alpine")
            .with_exposed_port(ContainerPort::Tcp(5432))
            .with_wait_for(WaitFor::message_on_stdout(
                "database system is ready to accept connections",
            ));

        let request = base_image
            .with_env_var("POSTGRES_PASSWORD", "password")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_DB", "postgres");

        let container = request
            .start()
            .await
            .context("failed to start postgres container")?;
        let port = container
            .get_host_port_ipv4(ContainerPort::Tcp(5432))
            .await
            .context("failed to resolve postgres port")?;
        let url = format!("postgres://postgres:password@127.0.0.1:{port}/postgres");

        let config = {
            let mut attempts = 0;
            loop {
                match ConfigService::new(url.clone()).await {
                    Ok(service) => break service,
                    Err(error) => {
                        attempts += 1;
                        if attempts >= 10 {
                            return Err(error).context("failed to initialise config service");
                        }
                        sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        };
        let runtime = RuntimeStore::new(config.pool().clone())
            .await
            .context("failed to initialise runtime store")?;

        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let (_engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &bus,
            metrics.clone(),
            sample_fs_policy("/tmp/library"),
            sample_engine_profile(),
            Some(runtime.clone()),
        )
        .await
        .expect("failed to spawn orchestrator with runtime store");

        let torrent_id = Uuid::new_v4();
        orchestrator
            .handle_event(&Event::TorrentAdded {
                torrent_id,
                name: "demo".to_string(),
            })
            .await?;

        let statuses = runtime.load_statuses().await?;
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, torrent_id);

        orchestrator
            .handle_event(&Event::TorrentRemoved { torrent_id })
            .await?;

        assert!(runtime.load_statuses().await?.is_empty());

        config.pool().close().await;
        if !worker.is_finished() {
            worker.abort();
        }
        let _ = worker.await;
        drop(container);
        Ok(())
    }

    #[tokio::test]
    async fn orchestrator_applies_fsops_on_completion() -> Result<()> {
        let bus = EventBus::with_capacity(64);
        let metrics = Metrics::new()?;
        let temp = TempDir::new()?;
        let library_root = temp.path().join("library");
        let policy = sample_fs_policy(
            library_root
                .to_str()
                .expect("library root path should be valid UTF-8"),
        );
        let (engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &bus,
            metrics.clone(),
            policy.clone(),
            sample_engine_profile(),
            None,
        )
        .await
        .expect("failed to spawn orchestrator");

        let mut stream = bus.subscribe(None);
        orchestrator
            .add_torrent(AddTorrent {
                id: Uuid::new_v4(),
                source: TorrentSource::magnet("magnet:?xt=urn:btih:demo"),
                options: AddTorrentOptions {
                    name_hint: Some("demo".to_string()),
                    ..AddTorrentOptions::default()
                },
            })
            .await?;

        let torrent_id = Uuid::new_v4();
        let source_path = library_root.join("incoming").join("title");
        fs::create_dir_all(&source_path)?;
        fs::write(source_path.join("movie.mkv"), b"video-bytes")?;
        yield_now().await;
        engine.publish_completed(torrent_id, source_path.to_string_lossy().into_owned());

        timeout(Duration::from_secs(5), async {
            loop {
                let envelope = stream
                    .next()
                    .await
                    .ok_or_else(|| anyhow!("event stream closed before fsops completion"))?;
                match envelope.event {
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        return Ok::<(), anyhow::Error>(());
                    }
                    Event::FsopsFailed {
                        torrent_id: id,
                        ref message,
                    } if id == torrent_id => {
                        bail!("fsops failed unexpectedly: {message}");
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("fsops completion event")
        .expect("fsops pipeline failed unexpectedly");

        worker.abort();
        let _ = worker.await;

        let artifact_dir = PathBuf::from(&policy.library_root).join("title");
        assert!(artifact_dir.join("movie.mkv").exists());
        Ok(())
    }

    #[tokio::test]
    async fn orchestrator_reports_fsops_failures() {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new().expect("metrics registry");
        let (engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &bus,
            metrics.clone(),
            sample_fs_policy("   "),
            sample_engine_profile(),
            None,
        )
        .await
        .expect("failed to spawn orchestrator");
        let mut stream = bus.subscribe(None);
        let temp = TempDir::new().expect("tempdir");
        let source_path = temp.path().join("staging").join("title");
        fs::create_dir_all(&source_path).expect("staging path");
        fs::write(source_path.join("movie.mkv"), b"video").expect("write");

        let torrent_id = Uuid::new_v4();
        yield_now().await;
        engine.publish_completed(torrent_id, source_path.to_string_lossy().into_owned());
        timeout(Duration::from_secs(5), async {
            loop {
                let envelope = stream
                    .next()
                    .await
                    .ok_or_else(|| anyhow!("event stream closed before fsops failure"))?;
                match envelope.event {
                    Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id => {
                        return Ok::<(), anyhow::Error>(());
                    }
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        bail!("fsops completed unexpectedly");
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("fsops failure event")
        .expect("event stream closed before fsops failure");
        let result = orchestrator.apply_fsops(torrent_id).await;
        assert!(result.is_err(), "expected fsops to fail for blank policy");
        worker.abort();
        let _ = worker.await;
    }

    #[tokio::test]
    async fn orchestrator_updates_policy_dynamically() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let (engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &bus,
            metrics.clone(),
            sample_fs_policy("   "),
            sample_engine_profile(),
            None,
        )
        .await
        .expect("failed to spawn orchestrator");
        let mut stream = bus.subscribe(None);

        orchestrator
            .update_fs_policy(sample_fs_policy("/tmp/library"))
            .await;

        let temp = TempDir::new()?;
        let staged = temp.path().join("stage");
        fs::create_dir_all(&staged)?;
        fs::write(staged.join("movie.mkv"), b"video")?;

        let torrent_id = Uuid::new_v4();
        yield_now().await;
        engine.publish_completed(torrent_id, staged.to_string_lossy().into_owned());
        timeout(Duration::from_secs(5), async {
            loop {
                if orchestrator.catalog.get(torrent_id).await.is_some() {
                    return Ok::<(), anyhow::Error>(());
                }
                yield_now().await;
            }
        })
        .await
        .expect("catalog entry ready")?;
        orchestrator.apply_fsops(torrent_id).await?;
        timeout(Duration::from_secs(5), async {
            loop {
                let envelope = stream
                    .next()
                    .await
                    .ok_or_else(|| anyhow!("event stream closed before fsops completion"))?;
                match envelope.event {
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        return Ok::<(), anyhow::Error>(());
                    }
                    Event::FsopsFailed {
                        torrent_id: id,
                        ref message,
                    } if id == torrent_id => {
                        bail!("fsops failed unexpectedly: {message}");
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("fsops completion event after policy update")
        .expect("fsops pipeline failed unexpectedly after policy update");

        worker.abort();
        let _ = worker.await;
        Ok(())
    }
}

#[cfg(test)]
mod engine_refresh_tests {
    use super::*;
    use revaer_events::EventBus;
    use revaer_torrent_core::{
        AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit, TorrentWorkflow,
    };
    use serde_json::json;
    use tokio::sync::RwLock;

    #[derive(Default)]
    struct RecordingEngine {
        applied: RwLock<Vec<EngineProfile>>,
        removed: RwLock<Vec<(Uuid, RemoveTorrent)>>,
        paused: RwLock<Vec<Uuid>>,
        resumed: RwLock<Vec<Uuid>>,
        sequential: RwLock<Vec<(Uuid, bool)>>,
        limits: RwLock<Vec<(Option<Uuid>, TorrentRateLimit)>>,
        selections: RwLock<Vec<(Uuid, FileSelectionUpdate)>>,
        reannounced: RwLock<Vec<Uuid>>,
        rechecked: RwLock<Vec<Uuid>>,
    }

    #[async_trait]
    impl TorrentEngine for RecordingEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()> {
            self.removed.write().await.push((id, options));
            Ok(())
        }

        async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.paused.write().await.push(id);
            Ok(())
        }

        async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.resumed.write().await.push(id);
            Ok(())
        }

        async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
            self.sequential.write().await.push((id, sequential));
            Ok(())
        }

        async fn update_limits(
            &self,
            id: Option<Uuid>,
            limits: TorrentRateLimit,
        ) -> anyhow::Result<()> {
            self.limits.write().await.push((id, limits));
            Ok(())
        }

        async fn update_selection(
            &self,
            id: Uuid,
            rules: FileSelectionUpdate,
        ) -> anyhow::Result<()> {
            self.selections.write().await.push((id, rules));
            Ok(())
        }

        async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
            self.reannounced.write().await.push(id);
            Ok(())
        }

        async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
            self.rechecked.write().await.push(id);
            Ok(())
        }
    }

    #[async_trait]
    impl EngineConfigurator for RecordingEngine {
        async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()> {
            self.applied.write().await.push(profile.clone());
            Ok(())
        }
    }

    fn sample_fs_policy() -> FsPolicy {
        let root = std::env::temp_dir().join(format!("revaer-fsops-{}", Uuid::new_v4()));
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.display().to_string(),
            extract: false,
            par2: "disabled".to_string(),
            flatten: false,
            move_mode: "copy".to_string(),
            cleanup_keep: json!([]),
            cleanup_drop: json!([]),
            chmod_file: None,
            chmod_dir: None,
            owner: None,
            group: None,
            umask: None,
            allow_paths: json!([]),
        }
    }

    fn engine_profile(label: &str) -> EngineProfile {
        EngineProfile {
            id: Uuid::new_v4(),
            implementation: format!("libtorrent-{label}"),
            listen_port: Some(6_881),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/tmp/resume".to_string(),
            download_root: "/downloads".to_string(),
            tracker: json!([]),
        }
    }

    #[tokio::test]
    async fn update_engine_profile_notifies_engine() -> Result<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus.clone(),
            sample_fs_policy(),
            engine_profile("initial"),
            None,
        ));

        let mut updated = engine_profile("updated");
        updated.max_download_bps = Some(1_500_000);
        updated.max_upload_bps = Some(750_000);
        orchestrator
            .update_engine_profile(updated.clone())
            .await
            .expect("profile update");

        let applied_profiles = {
            let guard = engine.applied.read().await;
            guard.clone()
        };
        assert_eq!(applied_profiles.len(), 1);
        assert_eq!(applied_profiles[0].implementation, updated.implementation);
        assert_eq!(applied_profiles[0].listen_port, updated.listen_port);

        let recorded_limits = {
            let guard = engine.limits.read().await;
            guard.clone()
        };
        assert_eq!(recorded_limits.len(), 1);
        assert!(
            recorded_limits[0].0.is_none(),
            "expected global rate limit update"
        );
        assert_eq!(recorded_limits[0].1.download_bps, Some(1_500_000));
        assert_eq!(recorded_limits[0].1.upload_bps, Some(750_000));
        Ok(())
    }

    #[tokio::test]
    async fn workflow_operations_forward_to_engine() -> Result<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("ops"),
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let limit = TorrentRateLimit {
            download_bps: Some(1_000),
            upload_bps: Some(500),
        };
        let selection = FileSelectionUpdate {
            include: vec!["*.mkv".to_string()],
            exclude: vec!["*.tmp".to_string()],
            skip_fluff: true,
            ..FileSelectionUpdate::default()
        };

        TorrentWorkflow::pause_torrent(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::resume_torrent(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::set_sequential(&*orchestrator, torrent_id, true).await?;
        TorrentWorkflow::update_limits(&*orchestrator, Some(torrent_id), limit.clone()).await?;
        TorrentWorkflow::update_limits(&*orchestrator, None, limit.clone()).await?;
        TorrentWorkflow::update_selection(&*orchestrator, torrent_id, selection.clone()).await?;
        TorrentWorkflow::reannounce(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::recheck(&*orchestrator, torrent_id).await?;
        TorrentWorkflow::remove_torrent(
            &*orchestrator,
            torrent_id,
            RemoveTorrent { with_data: true },
        )
        .await?;

        assert_eq!(engine.paused.read().await.len(), 1);
        assert_eq!(engine.resumed.read().await.len(), 1);
        assert_eq!(engine.sequential.read().await.len(), 1);
        assert_eq!(engine.limits.read().await.len(), 2);
        assert_eq!(engine.selections.read().await.len(), 1);
        assert_eq!(engine.reannounced.read().await.len(), 1);
        assert_eq!(engine.rechecked.read().await.len(), 1);
        assert_eq!(engine.removed.read().await.len(), 1);
        Ok(())
    }
}
