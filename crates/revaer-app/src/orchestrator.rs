use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use revaer_config::{EngineProfile, FsPolicy};
use revaer_events::{Event, EventBus, TorrentState};
use revaer_fsops::FsOpsService;
use revaer_torrent_core::{
    AddTorrent, FilePriority, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentFile,
    TorrentInspector, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentStatus,
    TorrentWorkflow,
};
use tokio::{sync::RwLock, task::JoinHandle};
use tracing::{error, info};
use uuid::Uuid;

#[async_trait]
pub trait EngineConfigurator: Send + Sync {
    async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()>;
}

#[cfg(feature = "libtorrent")]
use revaer_torrent_libt::LibtorrentEngine;

#[cfg(feature = "libtorrent")]
#[async_trait]
impl EngineConfigurator for LibtorrentEngine {
    async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()> {
        info!(
            implementation = %profile.implementation,
            listen_port = ?profile.listen_port,
            "applying engine profile update"
        );
        Ok(())
    }
}

/// Coordinates torrent engine lifecycle with filesystem post-processing via the shared event bus.
#[cfg_attr(not(test), allow(dead_code))]
pub struct TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    engine: Arc<E>,
    fsops: FsOpsService,
    events: EventBus,
    fs_policy: Arc<RwLock<FsPolicy>>,
    engine_profile: Arc<RwLock<EngineProfile>>,
    catalog: Arc<TorrentCatalog>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<E> TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    /// Construct a new orchestrator with shared dependencies.
    #[must_use]
    pub fn new(
        engine: Arc<E>,
        fsops: FsOpsService,
        events: EventBus,
        fs_policy: FsPolicy,
        engine_profile: EngineProfile,
    ) -> Self {
        Self {
            engine,
            fsops,
            events,
            fs_policy: Arc::new(RwLock::new(fs_policy)),
            engine_profile: Arc::new(RwLock::new(engine_profile)),
            catalog: Arc::new(TorrentCatalog::new()),
        }
    }

    /// Delegate torrent admission to the engine.
    pub async fn add_torrent(&self, request: AddTorrent) -> Result<()> {
        self.engine.add_torrent(request).await
    }

    /// Apply the filesystem policy to a completed torrent.
    pub async fn apply_fsops(&self, torrent_id: Uuid) -> Result<()> {
        let policy = self.fs_policy.read().await.clone();
        self.fsops.apply_policy(torrent_id, &policy)
    }

    async fn handle_event(&self, event: &Event) -> Result<()> {
        self.catalog.observe(event).await;
        if let Event::Completed { torrent_id, .. } = event {
            self.apply_fsops(*torrent_id).await?;
        }
        Ok(())
    }

    /// Spawn a background task that reacts to completion events and triggers filesystem processing.
    pub fn spawn_post_processing(self: &Arc<Self>) -> JoinHandle<()> {
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
    pub async fn update_fs_policy(&self, policy: FsPolicy) {
        let mut guard = self.fs_policy.write().await;
        *guard = policy;
    }

    /// Update the engine profile and propagate changes to the underlying engine.
    pub async fn update_engine_profile(&self, profile: EngineProfile) -> Result<()> {
        {
            let mut guard = self.engine_profile.write().await;
            *guard = profile.clone();
        }
        self.engine.apply_engine_profile(&profile).await
    }

    /// Remove the torrent from the engine.
    pub async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.engine.remove_torrent(id, options).await
    }
}

#[cfg(feature = "libtorrent")]
pub async fn spawn_libtorrent_orchestrator(
    events: &EventBus,
    fs_policy: FsPolicy,
    engine_profile: EngineProfile,
) -> Result<(
    Arc<LibtorrentEngine>,
    Arc<TorrentOrchestrator<LibtorrentEngine>>,
    JoinHandle<()>,
)> {
    let engine = Arc::new(LibtorrentEngine::new(events.clone()));
    engine.apply_engine_profile(&engine_profile).await?;
    let fsops = FsOpsService::new(events.clone());
    let orchestrator = Arc::new(TorrentOrchestrator::new(
        Arc::clone(&engine),
        fsops,
        events.clone(),
        fs_policy,
        engine_profile,
    ));
    let worker = orchestrator.spawn_post_processing();
    Ok((engine, orchestrator, worker))
}

#[async_trait]
impl<E> TorrentWorkflow for TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
        TorrentOrchestrator::add_torrent(self, request).await
    }

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()> {
        TorrentOrchestrator::remove_torrent(self, id, options).await
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

    #[allow(clippy::too_many_lines)]
    async fn observe(&self, event: &Event) {
        let mut guard = self.entries.write().await;
        match event {
            Event::TorrentAdded { torrent_id, name } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.name = Some(name.clone());
                entry.state = TorrentState::Queued;
                entry.progress = TorrentProgress::default();
                entry.library_path = None;
                entry.rates = TorrentRates::default();
                entry.added_at = Utc::now();
                entry.last_updated = Utc::now();
            }
            Event::FilesDiscovered { torrent_id, files } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                let mapped: Vec<TorrentFile> = files
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
                    .collect();
                entry.files = Some(mapped);
                entry.last_updated = Utc::now();
            }
            Event::Progress {
                torrent_id,
                bytes_downloaded,
                bytes_total,
            } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.progress.bytes_downloaded = *bytes_downloaded;
                entry.progress.bytes_total = *bytes_total;
                entry.progress.eta_seconds = None;
                entry.rates.download_bps = 0;
                entry.rates.upload_bps = 0;
                #[allow(clippy::cast_precision_loss)]
                let ratio = if *bytes_total == 0 {
                    0.0
                } else {
                    (*bytes_downloaded as f64) / (*bytes_total as f64)
                };
                entry.rates.ratio = ratio;
                entry.last_updated = Utc::now();
            }
            Event::StateChanged { torrent_id, state } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.state = state.clone();
                entry.last_updated = Utc::now();
            }
            Event::Completed {
                torrent_id,
                library_path,
            } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.state = TorrentState::Completed;
                entry.library_path = Some(library_path.clone());
                entry.completed_at = Some(Utc::now());
                entry.last_updated = Utc::now();
            }
            Event::FsopsFailed {
                torrent_id,
                message,
            } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.state = TorrentState::Failed {
                    message: message.clone(),
                };
                entry.last_updated = Utc::now();
            }
            Event::FsopsStarted { torrent_id } | Event::FsopsCompleted { torrent_id } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.last_updated = Utc::now();
            }
            Event::FsopsProgress { torrent_id, .. } => {
                let entry = guard
                    .entry(*torrent_id)
                    .or_insert_with(|| Self::blank_status(*torrent_id));
                entry.last_updated = Utc::now();
            }
            Event::SettingsChanged { .. } | Event::HealthChanged { .. } => {}
        }
    }

    async fn list(&self) -> Vec<TorrentStatus> {
        let guard = self.entries.read().await;
        let mut values: Vec<_> = guard.values().cloned().collect();
        values.sort_by(Self::compare_status);
        values
    }

    async fn get(&self, id: Uuid) -> Option<TorrentStatus> {
        let guard = self.entries.read().await;
        guard.get(&id).cloned()
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
}

#[cfg(all(test, feature = "libtorrent"))]
mod tests {
    use super::*;
    use revaer_events::{Event, EventBus};
    use revaer_torrent_core::{AddTorrent, AddTorrentOptions, TorrentSource};
    use serde_json::json;
    use tokio::{
        task::yield_now,
        time::{Duration, timeout},
    };

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
    async fn orchestrator_applies_fsops_on_completion() -> Result<()> {
        let bus = EventBus::with_capacity(64);
        let (engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &bus,
            sample_fs_policy("/library/media"),
            sample_engine_profile(),
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
        yield_now().await;
        engine.publish_completed(torrent_id, "/library/media/title");

        let mut fsops_completed = false;
        let mut fsops_started = false;
        for _ in 0..8 {
            let envelope = timeout(Duration::from_millis(500), stream.next())
                .await
                .expect("event stream timed out")
                .expect("event stream closed unexpectedly");
            match envelope.event {
                Event::FsopsCompleted { torrent_id: id } => {
                    if id == torrent_id {
                        fsops_completed = true;
                        break;
                    }
                }
                Event::FsopsStarted { torrent_id: id } => {
                    if id == torrent_id {
                        fsops_started = true;
                    }
                }
                _ => {}
            }
        }

        worker.abort();
        let _ = worker.await;

        assert!(
            fsops_started,
            "expected FsopsStarted event for completed torrent"
        );
        assert!(
            fsops_completed,
            "expected FsopsCompleted event for completed torrent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn orchestrator_reports_fsops_failures() {
        let bus = EventBus::with_capacity(16);
        let (_engine, orchestrator, worker) =
            spawn_libtorrent_orchestrator(&bus, sample_fs_policy("   "), sample_engine_profile())
                .await
                .expect("failed to spawn orchestrator");
        let mut stream = bus.subscribe(None);

        let torrent_id = Uuid::new_v4();
        let result = orchestrator.apply_fsops(torrent_id).await;
        assert!(result.is_err(), "expected fsops to fail for blank policy");

        let mut saw_failure = false;
        for _ in 0..3 {
            let envelope = timeout(Duration::from_millis(100), stream.next())
                .await
                .expect("event stream timed out")
                .expect("event stream closed unexpectedly");
            if matches!(
                envelope.event,
                Event::FsopsFailed { torrent_id: id, .. } if id == torrent_id
            ) {
                saw_failure = true;
                break;
            }
        }

        assert!(saw_failure, "expected FsopsFailed event for invalid policy");
        worker.abort();
        let _ = worker.await;
    }

    #[tokio::test]
    async fn orchestrator_updates_policy_dynamically() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let (_engine, orchestrator, worker) =
            spawn_libtorrent_orchestrator(&bus, sample_fs_policy("   "), sample_engine_profile())
                .await
                .expect("failed to spawn orchestrator");
        let mut stream = bus.subscribe(None);

        orchestrator
            .update_fs_policy(sample_fs_policy("/tmp/library"))
            .await;

        let torrent_id = Uuid::new_v4();
        orchestrator.apply_fsops(torrent_id).await?;

        let started = timeout(Duration::from_millis(200), stream.next())
            .await?
            .expect("event stream closed")
            .event;
        assert!(matches!(
            started,
            Event::FsopsStarted { torrent_id: id } if id == torrent_id
        ));

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
            let mut guard = self.removed.write().await;
            guard.push((id, options));
            Ok(())
        }

        async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            let mut guard = self.paused.write().await;
            guard.push(id);
            Ok(())
        }

        async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            let mut guard = self.resumed.write().await;
            guard.push(id);
            Ok(())
        }

        async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
            let mut guard = self.sequential.write().await;
            guard.push((id, sequential));
            Ok(())
        }

        async fn update_limits(
            &self,
            id: Option<Uuid>,
            limits: TorrentRateLimit,
        ) -> anyhow::Result<()> {
            let mut guard = self.limits.write().await;
            guard.push((id, limits));
            Ok(())
        }

        async fn update_selection(
            &self,
            id: Uuid,
            rules: FileSelectionUpdate,
        ) -> anyhow::Result<()> {
            let mut guard = self.selections.write().await;
            guard.push((id, rules));
            Ok(())
        }

        async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
            let mut guard = self.reannounced.write().await;
            guard.push(id);
            Ok(())
        }

        async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
            let mut guard = self.rechecked.write().await;
            guard.push(id);
            Ok(())
        }
    }

    #[async_trait]
    impl EngineConfigurator for RecordingEngine {
        async fn apply_engine_profile(&self, profile: &EngineProfile) -> Result<()> {
            let mut guard = self.applied.write().await;
            guard.push(profile.clone());
            Ok(())
        }
    }

    fn sample_fs_policy() -> FsPolicy {
        FsPolicy {
            id: Uuid::new_v4(),
            library_root: "/library".to_string(),
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
        let fsops = FsOpsService::new(bus.clone());
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus.clone(),
            sample_fs_policy(),
            engine_profile("initial"),
        ));

        let updated = engine_profile("updated");
        orchestrator
            .update_engine_profile(updated.clone())
            .await
            .expect("profile update");

        let applied = engine.applied.read().await;
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].implementation, updated.implementation);
        assert_eq!(applied[0].listen_port, updated.listen_port);
        Ok(())
    }

    #[tokio::test]
    async fn workflow_operations_forward_to_engine() -> Result<()> {
        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let fsops = FsOpsService::new(bus.clone());
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            engine_profile("ops"),
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
