//! Torrent orchestrator that wires libtorrent engine events into filesystem
//! post-processing and runtime persistence.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::engine_config::EngineRuntimePlan;
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
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use uuid::Uuid;

#[async_trait]
pub(crate) trait EngineConfigurator: Send + Sync {
    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> Result<()>;
}

#[cfg(feature = "libtorrent")]
use revaer_torrent_libt::LibtorrentEngine;

#[cfg(feature = "libtorrent")]
/// Dependencies required to spawn a libtorrent-backed orchestrator.
pub(crate) struct LibtorrentOrchestratorDeps {
    pub engine: Arc<LibtorrentEngine>,
    pub fsops: FsOpsService,
    pub runtime: Option<RuntimeStore>,
}

#[cfg(feature = "libtorrent")]
impl LibtorrentOrchestratorDeps {
    /// Build production dependencies using the shared event bus and metrics registry.
    pub(crate) fn new(
        events: &EventBus,
        metrics: &Metrics,
        runtime: Option<RuntimeStore>,
    ) -> Result<Self> {
        let engine = Arc::new(LibtorrentEngine::new(events.clone())?);
        let fsops = runtime.as_ref().map_or_else(
            || FsOpsService::new(events.clone(), metrics.clone()),
            |store| FsOpsService::new(events.clone(), metrics.clone()).with_runtime(store.clone()),
        );

        Ok(Self {
            engine,
            fsops,
            runtime,
        })
    }
}

#[cfg(feature = "libtorrent")]
#[async_trait]
impl EngineConfigurator for LibtorrentEngine {
    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> Result<()> {
        self.apply_runtime_config(plan.runtime.clone()).await
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
            while let Some(Ok(envelope)) = stream.next().await {
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
        let plan = EngineRuntimePlan::from_profile(&profile);
        for warning in &plan.effective.warnings {
            warn!(%warning, "engine profile guard rail applied");
        }

        info!(
            implementation = %profile.implementation,
            listen_port = ?plan.runtime.listen_port,
            max_active = ?plan.runtime.max_active,
            download_bps = ?plan.runtime.download_rate_limit,
            upload_bps = ?plan.runtime.upload_rate_limit,
            "applying engine profile update"
        );

        self.engine.apply_engine_plan(&plan).await?;
        self.engine
            .update_limits(None, plan.global_rate_limit())
            .await?;
        Ok(())
    }

    /// Remove the torrent from the engine.
    pub(crate) async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.engine.remove_torrent(id, options).await
    }
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
    fs_policy: FsPolicy,
    engine_profile: EngineProfile,
    deps: LibtorrentOrchestratorDeps,
) -> Result<(
    Arc<LibtorrentEngine>,
    Arc<TorrentOrchestrator<LibtorrentEngine>>,
    JoinHandle<()>,
)> {
    let LibtorrentOrchestratorDeps {
        engine,
        fsops,
        runtime,
    } = deps;
    let orchestrator = Arc::new(TorrentOrchestrator::new(
        Arc::clone(&engine),
        fsops,
        events.clone(),
        fs_policy,
        engine_profile,
        runtime.clone(),
    ));
    let initial_profile = orchestrator.engine_profile.read().await.clone();
    orchestrator.update_engine_profile(initial_profile).await?;
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
        let ratio = if bytes_total == 0 {
            0.0
        } else {
            bytes_to_f64(bytes_downloaded) / bytes_to_f64(bytes_total)
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

const fn bytes_to_f64(value: u64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "u64 to f64 conversion is needed for user-facing ratio reporting"
    )]
    {
        value as f64
    }
}

#[cfg(test)]
mod orchestrator_tests {
    use super::*;
    use anyhow::bail;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    use tokio::time::{Duration, timeout};
    use tokio_stream::StreamExt;

    #[derive(Default)]
    struct StubEngine;

    #[async_trait]
    impl TorrentEngine for StubEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl EngineConfigurator for StubEngine {
        async fn apply_engine_plan(&self, _plan: &EngineRuntimePlan) -> Result<()> {
            Ok(())
        }
    }

    fn sample_fs_policy(root: &Path) -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: root.join("library").display().to_string(),
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
            allow_paths: json!([root.display().to_string()]),
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
    async fn completed_event_triggers_fsops_pipeline() -> Result<()> {
        let temp = TempDir::new()?;
        let policy = sample_fs_policy(temp.path());
        let events = EventBus::with_capacity(16);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy.clone(),
            sample_engine_profile(),
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let source_path = temp.path().join("staging").join("title");
        fs::create_dir_all(&source_path)?;
        fs::write(source_path.join("movie.mkv"), b"video-bytes")?;
        let mut stream = events.subscribe(None);

        orchestrator
            .handle_event(&Event::Completed {
                torrent_id,
                library_path: source_path.to_string_lossy().into_owned(),
            })
            .await?;

        timeout(Duration::from_secs(5), async {
            while let Some(result) = stream.next().await {
                let envelope = result?;
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
            bail!("event stream closed before fsops completion observed");
        })
        .await??;

        let meta_path = PathBuf::from(&policy.library_root)
            .join(".revaer")
            .join(format!("{torrent_id}.meta.json"));
        assert!(
            meta_path.exists(),
            "fsops metadata should be written after completion"
        );
        Ok(())
    }

    #[tokio::test]
    async fn completed_event_with_invalid_policy_emits_failure() -> Result<()> {
        let temp = TempDir::new()?;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(events.clone(), metrics);
        let mut policy = sample_fs_policy(temp.path());
        policy.library_root = "   ".to_string();
        policy.allow_paths = json!([]);
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::new(StubEngine),
            fsops,
            events.clone(),
            policy,
            sample_engine_profile(),
            None,
        ));

        let torrent_id = Uuid::new_v4();
        let staged = temp.path().join("staging").join("title");
        fs::create_dir_all(&staged)?;
        fs::write(staged.join("movie.mkv"), b"video")?;
        let mut stream = events.subscribe(None);

        let result = orchestrator
            .handle_event(&Event::Completed {
                torrent_id,
                library_path: staged.to_string_lossy().into_owned(),
            })
            .await;
        assert!(result.is_err(), "invalid policy should surface an error");

        timeout(Duration::from_secs(3), async {
            while let Some(event) = stream.next().await {
                let envelope = event?;
                match envelope.event {
                    Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id => {
                        return Ok::<(), anyhow::Error>(());
                    }
                    Event::FsopsCompleted { torrent_id: id } if id == torrent_id => {
                        bail!("fsops unexpectedly succeeded with invalid policy");
                    }
                    _ => {}
                }
            }
            bail!("no fsops failure observed before stream closed");
        })
        .await??;
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
        applied: RwLock<Vec<EngineRuntimePlan>>,
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
        async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> Result<()> {
            self.applied.write().await.push(plan.clone());
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

        let applied_plans = {
            let guard = engine.applied.read().await;
            guard.clone()
        };
        assert_eq!(applied_plans.len(), 1);
        assert_eq!(
            applied_plans[0].effective.implementation,
            updated.implementation
        );
        assert_eq!(applied_plans[0].runtime.listen_port, updated.listen_port);
        assert_eq!(
            applied_plans[0].runtime.download_rate_limit,
            updated.max_download_bps
        );
        assert_eq!(
            applied_plans[0].runtime.upload_rate_limit,
            updated.max_upload_bps
        );

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
    async fn update_engine_profile_clamps_before_applying() -> Result<()> {
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

        let mut updated = engine_profile("guard");
        updated.max_download_bps = Some(revaer_config::MAX_RATE_LIMIT_BPS + 100);
        updated.download_root = String::new();
        updated.resume_dir = "  ".to_string();
        orchestrator.update_engine_profile(updated).await?;

        let applied_plans = engine.applied.read().await.clone();
        assert_eq!(
            applied_plans[0].runtime.download_rate_limit,
            Some(revaer_config::MAX_RATE_LIMIT_BPS)
        );
        assert_eq!(applied_plans[0].runtime.download_root, "/data/staging");
        assert_eq!(applied_plans[0].runtime.resume_dir, "/var/lib/revaer/state");
        assert!(
            applied_plans[0]
                .effective
                .warnings
                .iter()
                .any(|msg| msg.contains("guard rail")),
            "guard rail warnings should be propagated to the plan"
        );

        let recorded_limits = engine.limits.read().await.clone();
        assert_eq!(
            recorded_limits[0].1.download_bps,
            Some(u64::try_from(revaer_config::MAX_RATE_LIMIT_BPS)?)
        );
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

    #[test]
    fn event_torrent_id_extracts_supported_events() {
        let id = Uuid::new_v4();
        assert_eq!(
            event_torrent_id(&Event::TorrentAdded {
                torrent_id: id,
                name: "demo".into()
            }),
            Some(id)
        );
        assert_eq!(
            event_torrent_id(&Event::SelectionReconciled {
                torrent_id: id,
                reason: "policy".into()
            }),
            Some(id)
        );
        assert_eq!(
            event_torrent_id(&Event::HealthChanged { degraded: vec![] }),
            None,
            "health events should not carry torrent ids"
        );
    }

    #[tokio::test]
    async fn torrent_catalog_tracks_event_evolution() -> Result<()> {
        let catalog = TorrentCatalog::new();
        let id = Uuid::new_v4();
        let other = Uuid::new_v4();

        catalog
            .observe(&Event::TorrentAdded {
                torrent_id: id,
                name: "zeta".into(),
            })
            .await;
        catalog
            .observe(&Event::FilesDiscovered {
                torrent_id: id,
                files: vec![
                    DiscoveredFile {
                        path: "movie.mkv".into(),
                        size_bytes: 1_024,
                    },
                    DiscoveredFile {
                        path: "movie.srt".into(),
                        size_bytes: 512,
                    },
                ],
            })
            .await;
        catalog
            .observe(&Event::Progress {
                torrent_id: id,
                bytes_downloaded: 512,
                bytes_total: 1_024,
            })
            .await;
        catalog
            .observe(&Event::StateChanged {
                torrent_id: id,
                state: TorrentState::Downloading,
            })
            .await;
        catalog
            .observe(&Event::Completed {
                torrent_id: id,
                library_path: "/library/title".into(),
            })
            .await;
        catalog
            .observe(&Event::FsopsFailed {
                torrent_id: id,
                message: "oops".into(),
            })
            .await;

        catalog
            .observe(&Event::TorrentAdded {
                torrent_id: other,
                name: "alpha".into(),
            })
            .await;

        let mut statuses = catalog.list().await;
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses.remove(0).id, other, "sorted by name first");

        let status = catalog.get(id).await.expect("status");
        assert_eq!(status.progress.bytes_total, 1_024);
        assert!(matches!(status.state, TorrentState::Failed { .. }));
        let files = status.files.expect("files mapped");
        assert_eq!(files[0].index, 0);
        assert_eq!(files[1].index, 1);
        Ok(())
    }
}
