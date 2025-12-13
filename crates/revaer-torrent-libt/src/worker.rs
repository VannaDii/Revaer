//! Background task that drives the libtorrent session and emits events.

use crate::{
    command::EngineCommand,
    session::LibTorrentSession,
    store::{FastResumeStore, StoredTorrentMetadata},
    types::EngineRuntimeConfig,
};
use anyhow::Result;
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FilePriorityOverride, FileSelectionRules, FileSelectionUpdate,
    RemoveTorrent, TorrentRateLimit, TorrentRates, TorrentSource,
};
use std::collections::{BTreeSet, HashMap};
use std::convert::TryFrom;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

const ALERT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROGRESS_COALESCE_INTERVAL: Duration = Duration::from_millis(100);

/// Launch the background task that consumes engine commands and publishes events.
pub fn spawn(
    events: EventBus,
    mut commands: mpsc::Receiver<EngineCommand>,
    store: Option<FastResumeStore>,
    session: Box<dyn LibTorrentSession>,
) {
    tokio::spawn(async move {
        let mut worker = Worker::new(events, session, store);
        let mut poll = tokio::time::interval(ALERT_POLL_INTERVAL);
        loop {
            tokio::select! {
                command = commands.recv() => {
                    match command {
                        Some(command) => {
                            if let Err(err) = worker.handle(command).await {
                                let detail = err.to_string();
                                worker.mark_degraded("session", Some(&detail));
                                warn!(error = %err, "libtorrent command handling failed");
                            }
                        }
                        None => break,
                    }
                }
                _ = poll.tick() => {
                    if let Err(err) = worker.flush_session_events().await {
                        let detail = err.to_string();
                        worker.mark_degraded("session", Some(&detail));
                        warn!(error = %err, "libtorrent alert polling failed");
                    }
                }
            }
        }
        if let Err(err) = worker.flush_session_events().await {
            let detail = err.to_string();
            worker.mark_degraded("session", Some(&detail));
            warn!(error = %err, "libtorrent alert polling failed during shutdown");
        }
    });
}

struct Worker {
    events: EventBus,
    session: Box<dyn LibTorrentSession>,
    store: Option<FastResumeStore>,
    resume_cache: HashMap<Uuid, StoredTorrentMetadata>,
    fastresume_payloads: HashMap<Uuid, Vec<u8>>,
    health: BTreeSet<String>,
    progress_last_emit: HashMap<Uuid, Instant>,
    global_limits: TorrentRateLimit,
    per_torrent_limits: HashMap<Uuid, TorrentRateLimit>,
}

impl Worker {
    fn new(
        events: EventBus,
        session: Box<dyn LibTorrentSession>,
        store: Option<FastResumeStore>,
    ) -> Self {
        let mut resume_cache = HashMap::new();
        let mut fastresume_payloads = HashMap::new();
        let mut selection_reconciliations = Vec::new();
        let mut resume_store_warnings = Vec::new();
        let mut load_error: Option<String> = None;
        if let Some(store_ref) = &store {
            match store_ref.load_all() {
                Ok(states) => {
                    for state in states {
                        if let Some(metadata) = state.metadata {
                            resume_cache.insert(state.torrent_id, metadata);
                        } else {
                            selection_reconciliations.push((
                                state.torrent_id,
                                "metadata missing; defaulting selection rules".to_string(),
                            ));
                        }

                        if let Some(payload) = state.fastresume {
                            fastresume_payloads.insert(state.torrent_id, payload);
                        } else {
                            resume_store_warnings.push(format!(
                                "fastresume payload missing for torrent {}",
                                state.torrent_id
                            ));
                        }
                    }
                }
                Err(err) => {
                    load_error = Some(err.to_string());
                }
            }
        }

        let mut worker = Self {
            events,
            session,
            store,
            resume_cache,
            fastresume_payloads,
            health: BTreeSet::new(),
            progress_last_emit: HashMap::new(),
            global_limits: TorrentRateLimit::default(),
            per_torrent_limits: HashMap::new(),
        };

        if let Some(message) = load_error {
            worker.mark_degraded("resume_store", Some(&message));
        }

        for warning in resume_store_warnings {
            worker.mark_degraded("resume_store", Some(warning.as_str()));
        }

        for (torrent_id, reason) in selection_reconciliations {
            worker.publish_selection_reconciled(torrent_id, reason);
        }

        worker
    }

    async fn handle(&mut self, command: EngineCommand) -> Result<()> {
        match command {
            EngineCommand::Add(request) => self.handle_add(request).await?,
            EngineCommand::Remove { id, options } => self.handle_remove(id, options).await?,
            EngineCommand::Pause { id } => self.handle_pause(id).await?,
            EngineCommand::Resume { id } => self.handle_resume(id).await?,
            EngineCommand::SetSequential { id, sequential } => {
                self.handle_set_sequential(id, sequential).await?;
            }
            EngineCommand::UpdateLimits { id, limits } => {
                self.handle_update_limits(id, limits).await?;
            }
            EngineCommand::UpdateSelection { id, rules } => {
                self.handle_update_selection(id, rules).await?;
            }
            EngineCommand::Reannounce { id } => {
                self.handle_reannounce(id).await?;
            }
            EngineCommand::Recheck { id } => {
                self.handle_recheck(id).await?;
            }
            EngineCommand::ApplyConfig(config) => {
                self.handle_apply_config(config).await?;
            }
        }

        self.flush_session_events().await
    }

    async fn handle_add(&mut self, request: AddTorrent) -> Result<()> {
        let mut request = request;
        if let Some(stored) = self.resume_cache.get(&request.id) {
            if request.options.trackers.is_empty() && !stored.trackers.is_empty() {
                request.options.trackers.clone_from(&stored.trackers);
                request.options.replace_trackers = stored.replace_trackers;
            }
            if request.options.tags.is_empty() && !stored.tags.is_empty() {
                request.options.tags.clone_from(&stored.tags);
            }
        }

        self.session.add_torrent(&request).await?;
        if let Some(payload) = self.fastresume_payloads.get(&request.id).cloned() {
            if let Err(err) = self.session.load_fastresume(request.id, &payload).await {
                let detail = err.to_string();
                self.mark_degraded("resume_store", Some(&detail));
            } else {
                self.mark_recovered("resume_store");
            }
        }

        let mut effective_selection = request.options.file_rules.clone();
        let mut effective_download_dir = request.options.download_dir.clone();
        let mut effective_priorities: Vec<FilePriorityOverride> = Vec::new();
        let mut effective_sequential = request.options.sequential.unwrap_or(false);
        let mut reconciliation_reasons = Vec::new();

        if let Some(stored) = self.resume_cache.get(&request.id).cloned() {
            let selection_differs = stored.selection.include != effective_selection.include
                || stored.selection.exclude != effective_selection.exclude
                || stored.selection.skip_fluff != effective_selection.skip_fluff
                || !stored.priorities.is_empty();
            if selection_differs {
                let update = FileSelectionUpdate {
                    include: stored.selection.include.clone(),
                    exclude: stored.selection.exclude.clone(),
                    skip_fluff: stored.selection.skip_fluff,
                    priorities: stored.priorities.clone(),
                };
                self.session.update_selection(request.id, &update).await?;
                effective_selection = stored.selection.clone();
                effective_priorities.clear();
                effective_priorities.extend(stored.priorities.iter().cloned());
                reconciliation_reasons.push("restored persisted file selection".to_string());
            }

            if stored.sequential != effective_sequential {
                self.session
                    .set_sequential(request.id, stored.sequential)
                    .await?;
                effective_sequential = stored.sequential;
                reconciliation_reasons
                    .push("restored sequential flag from resume metadata".to_string());
            }

            if let Some(stored_dir) = stored.download_dir.clone()
                && effective_download_dir.as_deref() != Some(stored_dir.as_str())
            {
                effective_download_dir = Some(stored_dir);
                reconciliation_reasons
                    .push("restored download directory from resume metadata".to_string());
            }
        }

        for reason in reconciliation_reasons {
            self.publish_selection_reconciled(request.id, reason);
        }

        let name = request
            .options
            .name_hint
            .clone()
            .unwrap_or_else(|| format!("torrent-{}", request.id));
        let source_desc = match &request.source {
            TorrentSource::Magnet { uri } => {
                format!("magnet:{}", uri.chars().take(32).collect::<String>())
            }
            TorrentSource::Metainfo { .. } => "metainfo-bytes".to_string(),
        };
        info!(
            torrent_id = %request.id,
            torrent_name = %name,
            source = %source_desc,
            "torrent add command processed"
        );
        self.publish_torrent_added(&request);
        let selection = effective_selection;
        let download_dir = effective_download_dir;
        let priorities = effective_priorities;
        let sequential = effective_sequential;
        let trackers = request.options.trackers.clone();
        let replace_trackers = request.options.replace_trackers;
        let tags = request.options.tags.clone();
        self.update_metadata(request.id, move |meta| {
            meta.selection.clone_from(&selection);
            meta.download_dir.clone_from(&download_dir);
            meta.sequential = sequential;
            meta.priorities.clone_from(&priorities);
            meta.trackers.clone_from(&trackers);
            meta.replace_trackers = replace_trackers;
            meta.tags.clone_from(&tags);
        });
        Ok(())
    }

    async fn handle_remove(&mut self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.session.remove_torrent(id, &options).await?;
        let mut store_ok = true;
        if let Some(store) = &self.store {
            match store.remove(id) {
                Ok(()) => {}
                Err(err) => {
                    let detail = err.to_string();
                    self.mark_degraded("resume_store", Some(&detail));
                    warn!(
                        error = %detail,
                        torrent_id = %id,
                        "failed to prune fastresume artifacts after torrent removal"
                    );
                    store_ok = false;
                }
            }
        }
        info!(
            torrent_id = %id,
            with_data = options.with_data,
            "torrent remove command processed"
        );
        self.resume_cache.remove(&id);
        self.fastresume_payloads.remove(&id);
        self.progress_last_emit.remove(&id);
        self.per_torrent_limits.remove(&id);
        if store_ok {
            self.mark_recovered("resume_store");
        }
        Ok(())
    }

    async fn handle_pause(&mut self, id: Uuid) -> Result<()> {
        self.session.pause_torrent(id).await
    }

    async fn handle_resume(&mut self, id: Uuid) -> Result<()> {
        self.session.resume_torrent(id).await
    }

    async fn handle_set_sequential(&mut self, id: Uuid, sequential: bool) -> Result<()> {
        self.session.set_sequential(id, sequential).await?;
        debug!(torrent_id = %id, sequential, "updated sequential flag");
        self.update_metadata(id, move |meta| {
            meta.sequential = sequential;
        });
        Ok(())
    }

    async fn handle_update_limits(
        &mut self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> Result<()> {
        self.session.update_limits(id, &limits).await?;
        debug!(
            torrent_id = ?id,
            download_bps = ?limits.download_bps,
            upload_bps = ?limits.upload_bps,
            "updated rate limits"
        );
        if let Some(target) = id {
            self.per_torrent_limits.insert(target, limits.clone());
        } else {
            self.global_limits = limits.clone();
        }
        Ok(())
    }

    async fn handle_update_selection(
        &mut self,
        id: Uuid,
        rules: FileSelectionUpdate,
    ) -> Result<()> {
        self.session.update_selection(id, &rules).await?;
        debug!(
            torrent_id = %id,
            include = rules.include.len(),
            exclude = rules.exclude.len(),
            priorities = rules.priorities.len(),
            skip_fluff = rules.skip_fluff,
            "updated file selection"
        );
        let selection = FileSelectionRules {
            include: rules.include.clone(),
            exclude: rules.exclude.clone(),
            skip_fluff: rules.skip_fluff,
        };
        let priorities: Vec<FilePriorityOverride> = rules.priorities.clone();
        self.update_metadata(id, move |meta| {
            meta.selection.clone_from(&selection);
            meta.priorities.clone_from(&priorities);
        });
        Ok(())
    }

    async fn handle_reannounce(&mut self, id: Uuid) -> Result<()> {
        self.session.reannounce(id).await?;
        info!(torrent_id = %id, "reannounce requested");
        Ok(())
    }

    async fn handle_recheck(&mut self, id: Uuid) -> Result<()> {
        self.session.recheck(id).await?;
        info!(torrent_id = %id, "recheck requested");
        Ok(())
    }

    async fn handle_apply_config(&mut self, config: EngineRuntimeConfig) -> Result<()> {
        self.session.apply_config(&config).await?;
        self.global_limits = TorrentRateLimit {
            download_bps: map_limit(config.download_rate_limit),
            upload_bps: map_limit(config.upload_rate_limit),
        };
        info!(
            download_root = %config.download_root,
            resume_dir = %config.resume_dir,
            enable_dht = config.enable_dht,
            sequential_default = config.sequential_default,
            listen_port = ?config.listen_port,
            max_active = ?config.max_active,
            "applied engine runtime configuration"
        );
        Ok(())
    }

    async fn flush_session_events(&mut self) -> Result<()> {
        match self.session.poll_events().await {
            Ok(events) => {
                for event in events {
                    self.publish_engine_event(event);
                }
                self.mark_recovered("session");
                Ok(())
            }
            Err(err) => {
                let detail = err.to_string();
                self.mark_degraded("session", Some(&detail));
                Err(err)
            }
        }
    }

    fn publish_engine_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::FilesDiscovered { torrent_id, files } => {
                if files.is_empty() {
                    return;
                }
                let discovered: Vec<DiscoveredFile> = files
                    .into_iter()
                    .map(|file| DiscoveredFile {
                        path: file.path,
                        size_bytes: file.size_bytes,
                    })
                    .collect();
                let _ = self.events.publish(Event::FilesDiscovered {
                    torrent_id,
                    files: discovered,
                });
                self.mark_recovered("session");
            }
            EngineEvent::Progress {
                torrent_id,
                progress,
                rates,
            } => {
                if !self.should_emit_progress(torrent_id) {
                    debug!(
                        torrent_id = %torrent_id,
                        "suppressing progress update to honour coalescing budget"
                    );
                    return;
                }
                self.verify_rate_limits(torrent_id, &rates);
                let _ = self.events.publish(Event::Progress {
                    torrent_id,
                    bytes_downloaded: progress.bytes_downloaded,
                    bytes_total: progress.bytes_total,
                });
                self.mark_recovered("session");
            }
            EngineEvent::StateChanged { torrent_id, state } => {
                let failed = matches!(&state, TorrentState::Failed { .. });
                let _ = self
                    .events
                    .publish(Event::StateChanged { torrent_id, state });
                if !failed {
                    self.mark_recovered("session");
                }
            }
            EngineEvent::Completed {
                torrent_id,
                library_path,
            } => {
                let _ = self.events.publish(Event::StateChanged {
                    torrent_id,
                    state: TorrentState::Completed,
                });
                let _ = self.events.publish(Event::Completed {
                    torrent_id,
                    library_path,
                });
                self.mark_recovered("session");
            }
            EngineEvent::MetadataUpdated {
                torrent_id,
                mut download_dir,
                ..
            } => {
                self.update_metadata(torrent_id, move |meta| {
                    if let Some(dir) = download_dir.take() {
                        meta.download_dir = Some(dir);
                    }
                });
            }
            EngineEvent::ResumeData {
                torrent_id,
                payload,
            } => {
                self.persist_fastresume(torrent_id, payload);
            }
            EngineEvent::Error {
                torrent_id,
                message,
            } => {
                let _ = self.events.publish(Event::StateChanged {
                    torrent_id,
                    state: TorrentState::Failed { message },
                });
                self.mark_degraded("session", None);
            }
        }
    }

    fn publish_torrent_added(&self, request: &AddTorrent) {
        let name = request
            .options
            .name_hint
            .clone()
            .unwrap_or_else(|| format!("torrent-{}", request.id));
        let _ = self.events.publish(Event::TorrentAdded {
            torrent_id: request.id,
            name,
        });
    }

    fn update_metadata<F>(&mut self, torrent_id: Uuid, mutate: F)
    where
        F: FnOnce(&mut StoredTorrentMetadata),
    {
        let mut metadata = self
            .resume_cache
            .get(&torrent_id)
            .cloned()
            .unwrap_or_default();
        mutate(&mut metadata);

        if let Some(store) = &self.store {
            if let Err(err) = store.write_metadata(torrent_id, &metadata) {
                let detail = err.to_string();
                self.mark_degraded("resume_store", Some(&detail));
                warn!(
                    error = %detail,
                    torrent_id = %torrent_id,
                    "failed to persist torrent metadata"
                );
            } else {
                self.mark_recovered("resume_store");
            }
        }

        self.resume_cache.insert(torrent_id, metadata);
    }

    fn publish_selection_reconciled(&self, torrent_id: Uuid, reason: impl Into<String>) {
        let _ = self.events.publish(Event::SelectionReconciled {
            torrent_id,
            reason: reason.into(),
        });
    }

    fn persist_fastresume(&mut self, torrent_id: Uuid, payload: Vec<u8>) {
        if let Some(store) = &self.store {
            if let Err(err) = store.write_fastresume(torrent_id, &payload) {
                let detail = err.to_string();
                self.mark_degraded("resume_store", Some(&detail));
                warn!(
                    error = %detail,
                    torrent_id = %torrent_id,
                    "failed to persist fastresume payload"
                );
            } else {
                self.mark_recovered("resume_store");
            }
        }
        self.fastresume_payloads.insert(torrent_id, payload);
    }

    fn should_emit_progress(&mut self, torrent_id: Uuid) -> bool {
        let now = Instant::now();
        if let Some(last) = self.progress_last_emit.get_mut(&torrent_id) {
            if now.duration_since(*last) >= PROGRESS_COALESCE_INTERVAL {
                *last = now;
                true
            } else {
                false
            }
        } else {
            self.progress_last_emit.insert(torrent_id, now);
            true
        }
    }

    fn verify_rate_limits(&mut self, torrent_id: Uuid, rates: &TorrentRates) {
        let mut violated = false;
        let limit = self
            .per_torrent_limits
            .get(&torrent_id)
            .cloned()
            .or_else(|| {
                if self.global_limits.download_bps.is_some()
                    || self.global_limits.upload_bps.is_some()
                {
                    Some(self.global_limits.clone())
                } else {
                    None
                }
            });

        if let Some(limit) = limit {
            if let Some(max) = limit.download_bps
                && rates.download_bps > max
            {
                violated = true;
                let detail = format!(
                    "torrent {} download rate {}bps exceeds cap {}bps",
                    torrent_id, rates.download_bps, max
                );
                self.mark_degraded("rate_limiter", Some(detail.as_str()));
            }

            if let Some(max) = limit.upload_bps
                && rates.upload_bps > max
            {
                violated = true;
                let detail = format!(
                    "torrent {} upload rate {}bps exceeds cap {}bps",
                    torrent_id, rates.upload_bps, max
                );
                self.mark_degraded("rate_limiter", Some(detail.as_str()));
            }
        }

        if !violated {
            self.mark_recovered("rate_limiter");
        }
    }

    fn mark_degraded(&mut self, component: &str, detail: Option<&str>) {
        let inserted = self.health.insert(component.to_string());
        if inserted {
            let degraded = self.health.iter().cloned().collect::<Vec<_>>();
            let _ = self.events.publish(Event::HealthChanged { degraded });
            if let Some(detail) = detail {
                warn!(
                    component = component,
                    detail = %detail,
                    "engine component degraded"
                );
            } else {
                warn!(component = component, "engine component degraded");
            }
        } else if let Some(detail) = detail {
            warn!(
                component = component,
                detail = %detail,
                "engine component still degraded"
            );
        }
    }

    fn mark_recovered(&mut self, component: &str) {
        if self.health.remove(component) {
            let degraded = self.health.iter().cloned().collect::<Vec<_>>();
            let _ = self.events.publish(Event::HealthChanged { degraded });
            info!(component = component, "engine component recovered");
        }
    }
}

fn map_limit(value: Option<i64>) -> Option<u64> {
    value.and_then(|raw| {
        if raw >= 0 {
            u64::try_from(raw).ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::EngineCommand,
        session::StubSession,
        store::{FastResumeStore, StoredTorrentMetadata},
    };
    use anyhow::Result;
    use chrono::Utc;
    use revaer_events::{Event, EventBus};
    use revaer_torrent_core::{
        AddTorrent, AddTorrentOptions, FilePriority, FilePriorityOverride, FileSelectionRules,
        FileSelectionUpdate, RemoveTorrent, TorrentProgress, TorrentRates, TorrentSource,
    };
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout};
    use tokio_stream::StreamExt;

    async fn next_event_with_timeout(
        stream: &mut revaer_events::EventStream,
        timeout_ms: u64,
    ) -> Option<Event> {
        match timeout(Duration::from_millis(timeout_ms), stream.next()).await {
            Ok(Some(Ok(envelope))) => Some(envelope.event),
            _ => None,
        }
    }

    #[tokio::test]
    async fn add_command_with_stub_session_publishes_initial_events() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:stub"),
            options: AddTorrentOptions {
                name_hint: Some("example.torrent".into()),
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(descriptor.clone()))
            .await?;

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::TorrentAdded { torrent_id, name }) => {
                assert_eq!(torrent_id, descriptor.id);
                assert_eq!(name, "example.torrent");
            }
            other => panic!("expected torrent added event, got {other:?}"),
        }

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::StateChanged { torrent_id, state }) => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Queued));
            }
            other => panic!("expected queued state change, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn remove_command_emits_stopped_event() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:stub-remove"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(descriptor.clone()))
            .await?;
        // Drain initial torrent added + state events.
        let _ = next_event_with_timeout(&mut stream, 50).await;
        let _ = next_event_with_timeout(&mut stream, 50).await;

        worker
            .handle(EngineCommand::Remove {
                id: descriptor.id,
                options: RemoveTorrent { with_data: true },
            })
            .await?;

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::StateChanged { torrent_id, state }) => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Stopped));
            }
            other => panic!("expected stopped state change, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn pause_and_resume_commands_emit_expected_states() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:stub-pause"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(descriptor.clone()))
            .await?;
        let _ = next_event_with_timeout(&mut stream, 50).await;
        let _ = next_event_with_timeout(&mut stream, 50).await;

        worker
            .handle(EngineCommand::Pause { id: descriptor.id })
            .await?;
        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::StateChanged { torrent_id, state }) => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Stopped));
            }
            other => panic!("expected stopped state, got {other:?}"),
        }

        worker
            .handle(EngineCommand::Resume { id: descriptor.id })
            .await?;
        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::StateChanged { torrent_id, state }) => {
                assert_eq!(torrent_id, descriptor.id);
                assert!(matches!(state, TorrentState::Downloading));
            }
            other => panic!("expected downloading state, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn resume_metadata_reconciliation_persists_updates() -> Result<()> {
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());
        store.ensure_initialized()?;

        let torrent_id = Uuid::new_v4();
        let seed_metadata = StoredTorrentMetadata {
            selection: FileSelectionRules {
                include: vec!["movies/**".into()],
                exclude: vec!["extras/**".into()],
                skip_fluff: true,
            },
            priorities: vec![FilePriorityOverride {
                index: 0,
                priority: FilePriority::High,
            }],
            download_dir: Some("/persisted/downloads".into()),
            sequential: true,
            trackers: vec!["https://tracker.example/announce".into()],
            replace_trackers: true,
            tags: vec!["persisted".into()],
            updated_at: Utc::now(),
        };
        store.write_metadata(torrent_id, &seed_metadata)?;
        store.write_fastresume(torrent_id, br#"{"resume":"payload"}"#)?;

        let bus = EventBus::with_capacity(32);
        let mut stream = bus.subscribe(None);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, Some(store.clone()));

        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:stub-resume"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(descriptor.clone()))
            .await?;

        // Drain torrent added and initial state change events.
        let _ = next_event_with_timeout(&mut stream, 50).await;
        let _ = next_event_with_timeout(&mut stream, 50).await;
        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::SelectionReconciled { torrent_id, .. }) => {
                assert_eq!(torrent_id, descriptor.id);
            }
            other => panic!("expected selection reconciliation event, got {other:?}"),
        }

        // Apply a new selection update and toggle sequential mode.
        let update = FileSelectionUpdate {
            include: vec!["Season1/**".into()],
            exclude: vec!["extras/**".into()],
            skip_fluff: false,
            priorities: vec![FilePriorityOverride {
                index: 1,
                priority: FilePriority::Low,
            }],
        };

        worker
            .handle(EngineCommand::UpdateSelection {
                id: descriptor.id,
                rules: update.clone(),
            })
            .await?;
        worker
            .handle(EngineCommand::SetSequential {
                id: descriptor.id,
                sequential: false,
            })
            .await?;

        let entries = store.load_all()?;
        let persisted = entries
            .into_iter()
            .find(|entry| entry.torrent_id == descriptor.id)
            .expect("stored metadata present");
        let persisted_meta = persisted.metadata.expect("metadata persisted");
        assert_eq!(persisted_meta.selection.include, update.include);
        assert_eq!(persisted_meta.selection.exclude, update.exclude);
        assert_eq!(persisted_meta.priorities.len(), 1);
        assert!(!persisted_meta.sequential);
        assert_eq!(
            persisted_meta.trackers,
            vec!["https://tracker.example/announce".to_string()]
        );
        assert!(persisted_meta.replace_trackers);
        assert_eq!(persisted_meta.tags, vec!["persisted".to_string()]);
        assert!(
            persisted
                .fastresume
                .is_some_and(|payload| !payload.is_empty()),
            "expected non-empty fastresume payload"
        );

        Ok(())
    }

    #[tokio::test]
    async fn progress_updates_coalesced_to_ten_hz() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        worker.publish_engine_event(EngineEvent::Progress {
            torrent_id,
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1024,
                ..TorrentProgress::default()
            },
            rates: TorrentRates {
                download_bps: 2000,
                upload_bps: 500,
                ratio: 0.5,
            },
        });

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::Progress { torrent_id: id, .. }) => assert_eq!(id, torrent_id),
            other => panic!("expected progress event, got {other:?}"),
        }

        // Immediate second update should be suppressed to honour the coalescing budget.
        worker.publish_engine_event(EngineEvent::Progress {
            torrent_id,
            progress: TorrentProgress {
                bytes_downloaded: 768,
                bytes_total: 1024,
                ..TorrentProgress::default()
            },
            rates: TorrentRates {
                download_bps: 2500,
                upload_bps: 800,
                ratio: 0.75,
            },
        });

        assert!(
            next_event_with_timeout(&mut stream, 20).await.is_none(),
            "second event should have been coalesced"
        );

        // After the coalescing interval elapses the next event should be emitted.
        sleep(PROGRESS_COALESCE_INTERVAL).await;
        worker.publish_engine_event(EngineEvent::Progress {
            torrent_id,
            progress: TorrentProgress {
                bytes_downloaded: 900,
                bytes_total: 1024,
                ..TorrentProgress::default()
            },
            rates: TorrentRates {
                download_bps: 2200,
                upload_bps: 600,
                ratio: 0.88,
            },
        });

        assert!(
            matches!(
                next_event_with_timeout(&mut stream, 50).await,
                Some(Event::Progress { .. })
            ),
            "expected progress event after coalescing window"
        );

        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_violations_emit_health_degradation() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        worker
            .handle_update_limits(
                None,
                TorrentRateLimit {
                    download_bps: Some(1_000),
                    upload_bps: None,
                },
            )
            .await?;

        worker.publish_engine_event(EngineEvent::Progress {
            torrent_id,
            progress: TorrentProgress {
                bytes_downloaded: 100,
                bytes_total: 1_000,
                ..TorrentProgress::default()
            },
            rates: TorrentRates {
                download_bps: 2_000,
                upload_bps: 200,
                ratio: 0.1,
            },
        });

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert_eq!(degraded, vec!["rate_limiter"]);
            }
            other => panic!("expected health changed event, got {other:?}"),
        }

        // Drain the progress event that follows the degradation notification.
        assert!(
            matches!(
                next_event_with_timeout(&mut stream, 50).await,
                Some(Event::Progress { .. })
            ),
            "expected progress event"
        );

        sleep(PROGRESS_COALESCE_INTERVAL).await;

        // Subsequent event under the cap should clear the degradation.
        worker.publish_engine_event(EngineEvent::Progress {
            torrent_id,
            progress: TorrentProgress {
                bytes_downloaded: 400,
                bytes_total: 1_000,
                ..TorrentProgress::default()
            },
            rates: TorrentRates {
                download_bps: 900,
                upload_bps: 100,
                ratio: 0.4,
            },
        });

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert!(degraded.is_empty(), "expected recovery event");
            }
            other => panic!("expected health recovery event, got {other:?}"),
        }

        Ok(())
    }

    struct ErrorSession;

    #[async_trait::async_trait]
    impl LibTorrentSession for ErrorSession {
        async fn add_torrent(&mut self, _request: &AddTorrent) -> Result<()> {
            Ok(())
        }

        async fn remove_torrent(&mut self, _id: Uuid, _options: &RemoveTorrent) -> Result<()> {
            Ok(())
        }

        async fn pause_torrent(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn resume_torrent(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn set_sequential(&mut self, _id: Uuid, _sequential: bool) -> Result<()> {
            Ok(())
        }

        async fn load_fastresume(&mut self, _id: Uuid, _payload: &[u8]) -> Result<()> {
            Ok(())
        }

        async fn update_limits(
            &mut self,
            _id: Option<Uuid>,
            _limits: &TorrentRateLimit,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_selection(
            &mut self,
            _id: Uuid,
            _rules: &FileSelectionUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn reannounce(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn recheck(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn apply_config(&mut self, _config: &EngineRuntimeConfig) -> Result<()> {
            Ok(())
        }

        async fn poll_events(&mut self) -> Result<Vec<EngineEvent>> {
            Err(anyhow::anyhow!("simulated session failure"))
        }
    }

    #[tokio::test]
    async fn session_poll_failure_marks_engine_degraded() {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(ErrorSession);
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        assert!(worker.flush_session_events().await.is_err());

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert_eq!(degraded, vec!["session"]);
            }
            other => panic!("expected session degradation event, got {other:?}"),
        }
    }
}
