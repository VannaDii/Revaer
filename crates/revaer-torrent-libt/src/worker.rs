use crate::{
    command::EngineCommand,
    session::LibtSession,
    store::{FastResumeStore, StoredTorrentMetadata},
};
use anyhow::Result;
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FilePriorityOverride, FileSelectionRules, FileSelectionUpdate,
    RemoveTorrent, TorrentRateLimit, TorrentSource,
};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

const ALERT_POLL_INTERVAL: Duration = Duration::from_millis(200);

pub fn spawn(
    events: EventBus,
    mut commands: mpsc::Receiver<EngineCommand>,
    store: Option<FastResumeStore>,
    session: Box<dyn LibtSession>,
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
                                warn!(error = %err, "libtorrent command handling failed");
                            }
                        }
                        None => break,
                    }
                }
                _ = poll.tick() => {
                    if let Err(err) = worker.flush_session_events().await {
                        warn!(error = %err, "libtorrent alert polling failed");
                    }
                }
            }
        }
        if let Err(err) = worker.flush_session_events().await {
            warn!(error = %err, "libtorrent alert polling failed during shutdown");
        }
    });
}

struct Worker {
    events: EventBus,
    session: Box<dyn LibtSession>,
    store: Option<FastResumeStore>,
    resume_cache: HashMap<Uuid, StoredTorrentMetadata>,
    fastresume_payloads: HashMap<Uuid, Vec<u8>>,
    health: BTreeSet<String>,
}

impl Worker {
    fn new(
        events: EventBus,
        session: Box<dyn LibtSession>,
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
        }

        self.flush_session_events().await
    }

    async fn handle_add(&mut self, request: AddTorrent) -> Result<()> {
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

            if let Some(stored_dir) = stored.download_dir.clone() {
                if effective_download_dir.as_deref() != Some(stored_dir.as_str()) {
                    effective_download_dir = Some(stored_dir);
                    reconciliation_reasons
                        .push("restored download directory from resume metadata".to_string());
                }
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
            "libtorrent stub add command processed"
        );
        self.publish_torrent_added(&request);
        let selection = effective_selection;
        let download_dir = effective_download_dir;
        let priorities = effective_priorities;
        let sequential = effective_sequential;
        self.update_metadata(request.id, move |meta| {
            meta.selection.clone_from(&selection);
            meta.download_dir.clone_from(&download_dir);
            meta.sequential = sequential;
            meta.priorities.clone_from(&priorities);
        });
        Ok(())
    }

    async fn handle_remove(&mut self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.session.remove_torrent(id, &options).await?;
        let mut store_ok = true;
        if let Some(store) = &self.store {
            if let Err(err) = store.remove(id) {
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
        info!(
            torrent_id = %id,
            with_data = options.with_data,
            "libtorrent stub remove command processed"
        );
        self.resume_cache.remove(&id);
        self.fastresume_payloads.remove(&id);
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
        warn!(torrent_id = %id, "reannounce requested (stubbed)");
        Ok(())
    }

    async fn handle_recheck(&mut self, id: Uuid) -> Result<()> {
        self.session.recheck(id).await?;
        warn!(torrent_id = %id, "recheck requested (stubbed)");
        Ok(())
    }

    async fn flush_session_events(&mut self) -> Result<()> {
        let events = self.session.poll_events().await?;
        for event in events {
            self.publish_engine_event(event);
        }
        Ok(())
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
                ..
            } => {
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
