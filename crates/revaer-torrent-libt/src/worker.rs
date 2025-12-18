//! Background task that drives the libtorrent session and emits events.

use crate::{
    command::EngineCommand,
    session::LibTorrentSession,
    store::{FastResumeStore, StoredTorrentMetadata},
    types::{AltSpeedRuntimeConfig, AltSpeedSchedule, EngineRuntimeConfig},
};
use anyhow::{Result, bail};
use chrono::{DateTime, Datelike, Timelike, Utc};
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FilePriorityOverride, FileSelectionRules, FileSelectionUpdate,
    RemoveTorrent, StorageMode, TorrentFile, TorrentProgress, TorrentRateLimit, TorrentRates,
    TorrentSource,
    model::{TorrentOptionsUpdate, TorrentTrackersUpdate, TorrentWebSeedsUpdate, TrackerStatus},
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::TryFrom;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

const ALERT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROGRESS_COALESCE_INTERVAL: Duration = Duration::from_millis(100);
const ALT_SPEED_EVAL_INTERVAL: Duration = Duration::from_secs(30);

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
        let mut alt_tick = tokio::time::interval(ALT_SPEED_EVAL_INTERVAL);
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
                _ = alt_tick.tick() => {
                    if let Err(err) = worker.reconcile_alt_speed().await {
                        let detail = err.to_string();
                        worker.mark_degraded("session", Some(&detail));
                        warn!(error = %err, "failed to apply alt speed schedule");
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
    base_limits: TorrentRateLimit,
    global_limits: TorrentRateLimit,
    storage_mode: StorageMode,
    use_partfile: bool,
    per_torrent_limits: HashMap<Uuid, TorrentRateLimit>,
    seed_ratio_default: Option<f64>,
    seed_time_limit_default: Option<Duration>,
    seeding_goals: HashMap<Uuid, SeedingGoal>,
    alt_speed: Option<AltSpeedPlan>,
}

#[derive(Clone)]
struct MetadataReplaceFlags {
    replace_trackers: bool,
    replace_web_seeds: bool,
}

#[derive(Clone)]
struct AddMetadata {
    selection: FileSelectionRules,
    download_dir: Option<String>,
    priorities: Vec<FilePriorityOverride>,
    sequential: bool,
    seed_mode: Option<bool>,
    hash_check_sample_pct: Option<u8>,
    super_seeding: Option<bool>,
    trackers: Vec<String>,
    web_seeds: Vec<String>,
    replace: MetadataReplaceFlags,
    tags: Vec<String>,
    connections_limit: Option<i32>,
    rate_limit: Option<TorrentRateLimit>,
    seed_ratio_limit: Option<f64>,
    seed_time_limit: Option<u64>,
    auto_managed: Option<bool>,
    queue_position: Option<i32>,
    pex_enabled: Option<bool>,
    storage_mode: StorageMode,
    use_partfile: bool,
}

#[derive(Clone)]
struct AltSpeedPlan {
    limits: TorrentRateLimit,
    schedule: AltSpeedSchedule,
    active: bool,
}

#[derive(Debug, Clone)]
struct SeedingGoal {
    ratio_limit: Option<f64>,
    time_limit: Option<Duration>,
    seeding_since: Option<Instant>,
    satisfied: bool,
}

#[derive(Debug)]
enum PendingAction {
    PauseForSeedingGoal(Uuid),
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
            base_limits: TorrentRateLimit::default(),
            global_limits: TorrentRateLimit::default(),
            storage_mode: StorageMode::Sparse,
            use_partfile: true,
            per_torrent_limits: HashMap::new(),
            seed_ratio_default: None,
            seed_time_limit_default: None,
            seeding_goals: HashMap::new(),
            alt_speed: None,
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
            EngineCommand::Add(request) => self.handle_add(*request).await?,
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
            EngineCommand::UpdateOptions { id, options } => {
                self.handle_update_options(id, options).await?;
            }
            EngineCommand::UpdateTrackers { id, trackers } => {
                self.handle_update_trackers(id, trackers).await?;
            }
            EngineCommand::UpdateWebSeeds { id, web_seeds } => {
                self.handle_update_web_seeds(id, web_seeds).await?;
            }
            EngineCommand::Reannounce { id } => {
                self.handle_reannounce(id).await?;
            }
            EngineCommand::MoveStorage { id, download_dir } => {
                self.handle_move(id, download_dir).await?;
            }
            EngineCommand::Recheck { id } => {
                self.handle_recheck(id).await?;
            }
            EngineCommand::SetPieceDeadline {
                id,
                piece,
                deadline_ms,
            } => {
                self.handle_piece_deadline(id, piece, deadline_ms).await?;
            }
            EngineCommand::ApplyConfig(config) => {
                self.handle_apply_config(*config).await?;
            }
            EngineCommand::QueryPeers { id, respond_to } => {
                let result = self.session.peers(id).await;
                let _ = respond_to.send(result);
            }
        }

        self.flush_session_events().await
    }

    async fn handle_add(&mut self, request: AddTorrent) -> Result<()> {
        let mut request = request;
        self.backfill_request_from_resume(&mut request);

        if request.options.hash_check_sample_pct.is_some()
            && matches!(request.source, TorrentSource::Magnet { .. })
        {
            warn!(
                torrent_id = %request.id,
                "hash sample requested without metainfo; skipping preflight"
            );
            request.options.hash_check_sample_pct = None;
        }

        if request.options.seed_mode.unwrap_or(false)
            && request.options.hash_check_sample_pct.is_none()
        {
            warn!(
                torrent_id = %request.id,
                "seed mode requested without hash sample preflight; continuing without verification"
            );
        }

        self.session.add_torrent(&request).await?;
        self.apply_fastresume_if_present(request.id).await;

        let mut effective_selection = request.options.file_rules.clone();
        let mut effective_download_dir = request.options.download_dir.clone();
        let mut effective_priorities: Vec<FilePriorityOverride> = Vec::new();
        let mut effective_sequential = request.options.sequential.unwrap_or(false);
        let reconciliation_reasons = self
            .reconcile_from_resume(
                request.id,
                &mut effective_selection,
                &mut effective_download_dir,
                &mut effective_priorities,
                &mut effective_sequential,
            )
            .await?;

        self.publish_reconciliations(request.id, reconciliation_reasons);
        self.emit_add_event(&request);

        let seed_ratio_limit = request.options.seed_ratio_limit.or(self.seed_ratio_default);
        let seed_time_limit = request
            .options
            .seed_time_limit
            .map(Duration::from_secs)
            .or(self.seed_time_limit_default);
        let storage_mode = request.options.storage_mode.unwrap_or(self.storage_mode);

        self.persist_add_metadata(
            request.id,
            AddMetadata {
                selection: effective_selection,
                download_dir: effective_download_dir,
                storage_mode,
                use_partfile: self.use_partfile,
                priorities: effective_priorities,
                sequential: effective_sequential,
                seed_mode: request.options.seed_mode,
                hash_check_sample_pct: request.options.hash_check_sample_pct,
                super_seeding: request.options.super_seeding,
                trackers: request.options.trackers.clone(),
                web_seeds: request.options.web_seeds.clone(),
                replace: MetadataReplaceFlags {
                    replace_trackers: request.options.replace_trackers,
                    replace_web_seeds: request.options.replace_web_seeds,
                },
                tags: request.options.tags.clone(),
                connections_limit: request.options.connections_limit,
                rate_limit: rate_limit_for_metadata(&request.options.rate_limit),
                seed_ratio_limit,
                seed_time_limit: seed_time_limit.map(|value| value.as_secs()),
                auto_managed: request.options.auto_managed,
                queue_position: request.options.queue_position,
                pex_enabled: request.options.pex_enabled,
            },
        );
        self.apply_initial_rate_limit(request.id, &request.options.rate_limit)
            .await?;
        self.register_seeding_goal(request.id, seed_ratio_limit, seed_time_limit);
        Ok(())
    }

    fn backfill_request_from_resume(&self, request: &mut AddTorrent) {
        if let Some(stored) = self.resume_cache.get(&request.id) {
            if request.options.trackers.is_empty() && !stored.trackers.is_empty() {
                request.options.trackers.clone_from(&stored.trackers);
                request.options.replace_trackers = stored.replace_trackers;
            }
            if request.options.web_seeds.is_empty() && !stored.web_seeds.is_empty() {
                request.options.web_seeds.clone_from(&stored.web_seeds);
                request.options.replace_web_seeds = stored.replace_web_seeds;
            }
            if request.options.tags.is_empty() && !stored.tags.is_empty() {
                request.options.tags.clone_from(&stored.tags);
            }
            if request.options.connections_limit.is_none() {
                request.options.connections_limit = stored.connections_limit;
            }
            if !has_rate_limit(&request.options.rate_limit)
                && let Some(limit) = &stored.rate_limit
            {
                request.options.rate_limit = limit.clone();
            }
            if request.options.seed_ratio_limit.is_none() {
                request.options.seed_ratio_limit = stored.seed_ratio_limit;
            }
            if request.options.seed_time_limit.is_none() {
                request.options.seed_time_limit = stored.seed_time_limit;
            }
            if request.options.seed_mode.is_none() {
                request.options.seed_mode = stored.seed_mode;
            }
            if request.options.hash_check_sample_pct.is_none() {
                request.options.hash_check_sample_pct = stored.hash_check_sample_pct;
            }
            if request.options.super_seeding.is_none() {
                request.options.super_seeding = stored.super_seeding;
            }
            if request.options.auto_managed.is_none() {
                request.options.auto_managed = stored.auto_managed;
            }
            if request.options.queue_position.is_none() {
                request.options.queue_position = stored.queue_position;
            }
            if request.options.pex_enabled.is_none() {
                request.options.pex_enabled = stored.pex_enabled;
            }
            if request.options.storage_mode.is_none() {
                request.options.storage_mode = stored.storage_mode;
            }
            if request.options.download_dir.is_none() && stored.download_dir.is_some() {
                request
                    .options
                    .download_dir
                    .clone_from(&stored.download_dir);
            }
        }
    }

    async fn apply_fastresume_if_present(&mut self, id: Uuid) {
        if let Some(payload) = self.fastresume_payloads.get(&id).cloned() {
            if let Err(err) = self.session.load_fastresume(id, &payload).await {
                let detail = err.to_string();
                self.mark_degraded("resume_store", Some(&detail));
            } else {
                self.mark_recovered("resume_store");
            }
        }
    }

    async fn reconcile_from_resume(
        &mut self,
        torrent_id: Uuid,
        selection: &mut FileSelectionRules,
        download_dir: &mut Option<String>,
        priorities: &mut Vec<FilePriorityOverride>,
        sequential: &mut bool,
    ) -> Result<Vec<String>> {
        let mut reasons = Vec::new();
        if let Some(stored) = self.resume_cache.get(&torrent_id).cloned() {
            let selection_differs = stored.selection.include != selection.include
                || stored.selection.exclude != selection.exclude
                || stored.selection.skip_fluff != selection.skip_fluff
                || !stored.priorities.is_empty();
            if selection_differs {
                let update = FileSelectionUpdate {
                    include: stored.selection.include.clone(),
                    exclude: stored.selection.exclude.clone(),
                    skip_fluff: stored.selection.skip_fluff,
                    priorities: stored.priorities.clone(),
                };
                self.session.update_selection(torrent_id, &update).await?;
                *selection = stored.selection.clone();
                priorities.clear();
                priorities.extend(stored.priorities.iter().cloned());
                reasons.push("restored persisted file selection".to_string());
            }

            if stored.sequential != *sequential {
                self.session
                    .set_sequential(torrent_id, stored.sequential)
                    .await?;
                *sequential = stored.sequential;
                reasons.push("restored sequential flag from resume metadata".to_string());
            }

            if let Some(stored_dir) = stored.download_dir.clone()
                && download_dir.as_deref() != Some(stored_dir.as_str())
            {
                *download_dir = Some(stored_dir);
                reasons.push("restored download directory from resume metadata".to_string());
            }
        }
        Ok(reasons)
    }

    fn publish_reconciliations(&self, torrent_id: Uuid, reasons: Vec<String>) {
        for reason in reasons {
            self.publish_selection_reconciled(torrent_id, reason);
        }
    }

    async fn apply_initial_rate_limit(
        &mut self,
        torrent_id: Uuid,
        rate_limit: &TorrentRateLimit,
    ) -> Result<()> {
        if !has_rate_limit(rate_limit) {
            return Ok(());
        }

        self.session
            .update_limits(Some(torrent_id), rate_limit)
            .await?;
        self.per_torrent_limits
            .insert(torrent_id, rate_limit.clone());
        let rate_limit = rate_limit_for_metadata(rate_limit);
        self.update_metadata(torrent_id, move |meta| {
            meta.rate_limit = rate_limit;
        });
        Ok(())
    }

    fn emit_add_event(&self, request: &AddTorrent) {
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
        self.publish_torrent_added(request);
    }

    fn persist_add_metadata(&mut self, torrent_id: Uuid, metadata: AddMetadata) {
        let AddMetadata {
            selection,
            download_dir,
            storage_mode,
            use_partfile,
            priorities,
            sequential,
            seed_mode,
            hash_check_sample_pct,
            super_seeding,
            trackers,
            web_seeds,
            replace,
            tags,
            connections_limit,
            rate_limit,
            seed_ratio_limit,
            seed_time_limit,
            auto_managed,
            queue_position,
            pex_enabled,
        } = metadata;
        self.update_metadata(torrent_id, move |meta| {
            meta.selection.clone_from(&selection);
            meta.download_dir.clone_from(&download_dir);
            meta.storage_mode = Some(storage_mode);
            meta.use_partfile = Some(use_partfile);
            meta.sequential = sequential;
            meta.priorities.clone_from(&priorities);
            meta.trackers.clone_from(&trackers);
            meta.replace_trackers = replace.replace_trackers;
            meta.web_seeds.clone_from(&web_seeds);
            meta.replace_web_seeds = replace.replace_web_seeds;
            meta.tags.clone_from(&tags);
            meta.connections_limit = connections_limit;
            meta.rate_limit = rate_limit;
            meta.seed_mode = seed_mode;
            meta.hash_check_sample_pct = hash_check_sample_pct;
            meta.super_seeding = super_seeding;
            meta.seed_ratio_limit = seed_ratio_limit;
            meta.seed_time_limit = seed_time_limit;
            meta.auto_managed = auto_managed;
            meta.queue_position = queue_position;
            meta.pex_enabled = pex_enabled;
        });
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
        self.seeding_goals.remove(&id);
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
        self.update_metadata(id, |meta| {
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
            let rate_limit = rate_limit_for_metadata(&limits);
            self.update_metadata(target, move |meta| {
                meta.rate_limit = rate_limit;
            });
        } else {
            self.base_limits = limits.clone();
            self.global_limits = limits.clone();
            if let Some(plan) = &mut self.alt_speed {
                plan.active = false;
            }
            self.reconcile_alt_speed().await?;
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
        self.update_metadata(id, |meta| {
            meta.selection.clone_from(&selection);
            meta.priorities.clone_from(&priorities);
        });
        Ok(())
    }

    async fn handle_update_options(
        &mut self,
        id: Uuid,
        options: TorrentOptionsUpdate,
    ) -> Result<()> {
        let paused = options.paused;
        self.session.update_options(id, &options).await?;
        self.update_metadata(id, |meta| {
            if let Some(limit) = options.connections_limit {
                meta.connections_limit = (limit > 0).then_some(limit);
            }
            if let Some(pex_enabled) = options.pex_enabled {
                meta.pex_enabled = Some(pex_enabled);
            }
            if let Some(super_seeding) = options.super_seeding {
                meta.super_seeding = Some(super_seeding);
            }
            if let Some(auto_managed) = options.auto_managed {
                meta.auto_managed = Some(auto_managed);
            }
            if let Some(queue_position) = options.queue_position {
                meta.queue_position = Some(queue_position);
            }
            if let Some(seed_ratio_limit) = options.seed_ratio_limit {
                meta.seed_ratio_limit = Some(seed_ratio_limit);
            }
            if let Some(seed_time_limit) = options.seed_time_limit {
                meta.seed_time_limit = Some(seed_time_limit);
            }
        });
        let (ratio_limit, time_limit) = self.resume_cache.get(&id).map_or((None, None), |meta| {
            (meta.seed_ratio_limit, meta.seed_time_limit)
        });
        if let Some(paused_flag) = paused {
            if paused_flag {
                self.handle_pause(id).await?;
                info!(torrent_id = %id, "torrent paused via options update");
            } else {
                self.handle_resume(id).await?;
                info!(torrent_id = %id, "torrent resumed via options update");
            }
        }
        self.register_seeding_goal(id, ratio_limit, time_limit.map(Duration::from_secs));
        debug!(
            torrent_id = %id,
            connections_limit = ?options.connections_limit,
            paused = ?paused,
            pex_enabled = ?options.pex_enabled,
            super_seeding = ?options.super_seeding,
            auto_managed = ?options.auto_managed,
            queue_position = ?options.queue_position,
            seed_ratio_limit = ?options.seed_ratio_limit,
            seed_time_limit = ?options.seed_time_limit,
            "updated per-torrent options"
        );
        Ok(())
    }

    async fn handle_update_trackers(
        &mut self,
        id: Uuid,
        trackers: TorrentTrackersUpdate,
    ) -> Result<()> {
        self.session.update_trackers(id, &trackers).await?;
        let mut deduped = Vec::new();
        let mut seen = HashSet::new();
        for tracker in trackers.trackers {
            if seen.insert(tracker.clone()) {
                deduped.push(tracker);
            }
        }
        self.update_metadata(id, move |meta| {
            if trackers.replace {
                meta.trackers.clone_from(&deduped);
            } else {
                let mut merged = meta.trackers.clone();
                let mut merged_seen: HashSet<String> = merged.iter().cloned().collect();
                for tracker in deduped {
                    if merged_seen.insert(tracker.clone()) {
                        merged.push(tracker);
                    }
                }
                meta.trackers = merged;
            }
            meta.replace_trackers = trackers.replace;
        });
        Ok(())
    }

    async fn handle_update_web_seeds(
        &mut self,
        id: Uuid,
        web_seeds: TorrentWebSeedsUpdate,
    ) -> Result<()> {
        self.session.update_web_seeds(id, &web_seeds).await?;
        let mut deduped = Vec::new();
        let mut seen = HashSet::new();
        for seed in web_seeds.web_seeds {
            if seen.insert(seed.clone()) {
                deduped.push(seed);
            }
        }
        self.update_metadata(id, move |meta| {
            if web_seeds.replace {
                meta.web_seeds.clone_from(&deduped);
            } else {
                let mut merged = meta.web_seeds.clone();
                let mut merged_seen: HashSet<String> = merged.iter().cloned().collect();
                for seed in deduped {
                    if merged_seen.insert(seed.clone()) {
                        merged.push(seed);
                    }
                }
                meta.web_seeds = merged;
            }
            meta.replace_web_seeds = web_seeds.replace;
        });
        Ok(())
    }

    async fn handle_reannounce(&mut self, id: Uuid) -> Result<()> {
        self.session.reannounce(id).await?;
        info!(torrent_id = %id, "reannounce requested");
        Ok(())
    }

    async fn handle_move(&mut self, id: Uuid, download_dir: String) -> Result<()> {
        let target = download_dir.trim();
        if target.is_empty() {
            bail!("download directory is required for move");
        }
        self.session.move_torrent(id, target).await?;
        info!(torrent_id = %id, download_dir = %target, "move requested");
        Ok(())
    }

    async fn handle_recheck(&mut self, id: Uuid) -> Result<()> {
        self.session.recheck(id).await?;
        info!(torrent_id = %id, "recheck requested");
        Ok(())
    }

    async fn handle_piece_deadline(
        &mut self,
        id: Uuid,
        piece: u32,
        deadline_ms: Option<u32>,
    ) -> Result<()> {
        self.session
            .set_piece_deadline(id, piece, deadline_ms)
            .await?;
        info!(torrent_id = %id, piece = piece, "piece deadline updated");
        Ok(())
    }

    async fn handle_apply_config(&mut self, config: EngineRuntimeConfig) -> Result<()> {
        self.session.apply_config(&config).await?;
        self.base_limits = TorrentRateLimit {
            download_bps: map_limit(config.download_rate_limit),
            upload_bps: map_limit(config.upload_rate_limit),
        };
        self.global_limits = self.base_limits.clone();
        self.storage_mode = config.storage_mode.into();
        self.use_partfile = bool::from(config.use_partfile);
        self.alt_speed = config.alt_speed.clone().and_then(alt_speed_plan);
        self.seed_ratio_default = config.seed_ratio_limit;
        self.seed_time_limit_default = config
            .seed_time_limit
            .and_then(|seconds| u64::try_from(seconds).ok())
            .map(Duration::from_secs);
        info!(
            download_root = %config.download_root,
            resume_dir = %config.resume_dir,
            enable_dht = config.enable_dht,
            enable_lsd = bool::from(config.enable_lsd),
            enable_upnp = bool::from(config.enable_upnp),
            enable_natpmp = bool::from(config.enable_natpmp),
            enable_pex = bool::from(config.enable_pex),
            sequential_default = config.sequential_default,
            listen_port = ?config.listen_port,
            max_active = ?config.max_active,
            "applied engine runtime configuration"
        );
        self.reconcile_alt_speed().await?;
        Ok(())
    }

    async fn reconcile_alt_speed(&mut self) -> Result<()> {
        let now = Utc::now();
        self.reconcile_alt_speed_with_now(now).await
    }

    async fn reconcile_alt_speed_with_now(&mut self, now: DateTime<Utc>) -> Result<()> {
        let Some(plan) = self.alt_speed.as_mut() else {
            return Ok(());
        };
        let active = is_alt_speed_active(&plan.schedule, now);
        if active == plan.active {
            return Ok(());
        }

        let target = if active {
            plan.limits.clone()
        } else {
            self.base_limits.clone()
        };

        self.session.update_limits(None, &target).await?;
        self.global_limits = target.clone();
        plan.active = active;
        info!(
            active,
            start_minutes = plan.schedule.start_minutes,
            end_minutes = plan.schedule.end_minutes,
            "updated alternate speed limits"
        );
        Ok(())
    }

    async fn flush_session_events(&mut self) -> Result<()> {
        match self.session.poll_events().await {
            Ok(events) => {
                let mut actions = Vec::new();
                let mut saw_error = false;
                self.enqueue_time_based_goals(&mut actions);
                for event in events {
                    let is_error = matches!(
                        &event,
                        EngineEvent::Error { .. } | EngineEvent::SessionError { .. }
                    );
                    if is_error {
                        saw_error = true;
                    }
                    self.publish_engine_event(event, &mut actions);
                }
                if let Err(err) = self.apply_actions(actions).await {
                    let detail = err.to_string();
                    self.mark_degraded("session", Some(&detail));
                    return Err(err);
                }
                if !saw_error {
                    self.mark_recovered("session");
                }
                Ok(())
            }
            Err(err) => {
                let detail = err.to_string();
                self.mark_degraded("session", Some(&detail));
                Err(err)
            }
        }
    }

    fn publish_engine_event(&mut self, event: EngineEvent, actions: &mut Vec<PendingAction>) {
        match event {
            EngineEvent::FilesDiscovered { torrent_id, files } => {
                self.handle_files_discovered(torrent_id, files);
            }
            EngineEvent::Progress {
                torrent_id,
                progress,
                rates,
            } => {
                self.handle_progress_event(torrent_id, &progress, &rates, actions);
            }
            EngineEvent::StateChanged { torrent_id, state } => {
                self.handle_state_changed(torrent_id, state);
            }
            EngineEvent::Completed {
                torrent_id,
                library_path,
            } => {
                self.handle_completed(torrent_id, library_path, actions);
            }
            EngineEvent::MetadataUpdated {
                torrent_id,
                name,
                download_dir,
                ..
            } => {
                self.handle_metadata_updated(torrent_id, name, download_dir);
            }
            EngineEvent::ResumeData {
                torrent_id,
                payload,
            } => {
                self.handle_resume_data(torrent_id, payload);
            }
            EngineEvent::Error {
                torrent_id,
                message,
            } => {
                self.handle_error(torrent_id, message);
            }
            EngineEvent::TrackerStatus {
                torrent_id,
                trackers,
            } => {
                self.handle_tracker_status(torrent_id, trackers);
            }
            EngineEvent::SessionError { component, message } => {
                self.handle_session_error(component, &message);
            }
        }
    }

    fn handle_files_discovered(&mut self, torrent_id: Uuid, files: Vec<TorrentFile>) {
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

    fn handle_progress_event(
        &mut self,
        torrent_id: Uuid,
        progress: &TorrentProgress,
        rates: &TorrentRates,
        actions: &mut Vec<PendingAction>,
    ) {
        self.evaluate_seeding_goal(torrent_id, rates.ratio, actions);
        self.verify_rate_limits(torrent_id, rates);
        if !self.should_emit_progress(torrent_id) {
            debug!(
                torrent_id = %torrent_id,
                "suppressing progress update to honour coalescing budget"
            );
            return;
        }
        let _ = self.events.publish(Event::Progress {
            torrent_id,
            bytes_downloaded: progress.bytes_downloaded,
            bytes_total: progress.bytes_total,
        });
        self.mark_recovered("session");
    }

    fn handle_state_changed(&mut self, torrent_id: Uuid, state: TorrentState) {
        self.update_seeding_state(torrent_id, &state);
        let failed = matches!(&state, TorrentState::Failed { .. });
        let _ = self
            .events
            .publish(Event::StateChanged { torrent_id, state });
        if !failed {
            self.mark_recovered("session");
        }
    }

    fn handle_completed(
        &mut self,
        torrent_id: Uuid,
        library_path: String,
        actions: &mut Vec<PendingAction>,
    ) {
        self.update_seeding_state(torrent_id, &TorrentState::Completed);
        self.evaluate_seeding_goal(torrent_id, 0.0, actions);
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

    fn handle_metadata_updated(
        &mut self,
        torrent_id: Uuid,
        name: Option<String>,
        download_dir: Option<String>,
    ) {
        let download_dir_clone = download_dir.clone();
        self.update_metadata(torrent_id, move |meta| {
            meta.updated_at = Utc::now();
            if let Some(dir) = download_dir_clone.as_deref() {
                meta.download_dir = Some(dir.to_string());
            }
        });
        let _ = self.events.publish(Event::MetadataUpdated {
            torrent_id,
            name,
            download_dir,
        });
    }

    fn handle_resume_data(&mut self, torrent_id: Uuid, payload: Vec<u8>) {
        self.persist_fastresume(torrent_id, payload);
    }

    fn handle_error(&mut self, torrent_id: Uuid, message: String) {
        let detail = message.clone();
        let _ = self.events.publish(Event::StateChanged {
            torrent_id,
            state: TorrentState::Failed { message },
        });
        self.mark_degraded("session", Some(detail.as_str()));
    }

    fn handle_session_error(&mut self, component: Option<String>, message: &str) {
        if let Some(component) = component {
            self.mark_degraded(&component, Some(message));
        } else {
            self.mark_degraded("session", Some(message));
        }
    }

    fn handle_tracker_status(&mut self, torrent_id: Uuid, trackers: Vec<TrackerStatus>) {
        let mut has_error = false;
        let mut detail = None;
        for status in &trackers {
            if let Some(status_label) = status.status.as_deref()
                && status_label.eq_ignore_ascii_case("error")
            {
                has_error = true;
                if detail.is_none() {
                    detail.clone_from(&status.message);
                }
            }
        }
        self.update_metadata(torrent_id, move |meta| {
            for status in trackers {
                let Some(message) = status.message else {
                    continue;
                };
                meta.tracker_messages.insert(status.url, message);
            }
        });
        if has_error {
            if let Some(detail) = detail {
                self.mark_degraded("tracker", Some(detail.as_str()));
            } else {
                self.mark_degraded("tracker", Some("tracker reported error"));
            }
        } else {
            self.mark_recovered("tracker");
        }
    }

    fn register_seeding_goal(
        &mut self,
        torrent_id: Uuid,
        ratio_limit: Option<f64>,
        time_limit: Option<Duration>,
    ) {
        if ratio_limit.is_none() && time_limit.is_none() {
            self.seeding_goals.remove(&torrent_id);
            return;
        }
        self.seeding_goals.insert(
            torrent_id,
            SeedingGoal {
                ratio_limit,
                time_limit,
                seeding_since: None,
                satisfied: false,
            },
        );
    }

    fn update_seeding_state(&mut self, torrent_id: Uuid, state: &TorrentState) {
        if let Some(goal) = self.seeding_goals.get_mut(&torrent_id) {
            match state {
                TorrentState::Completed | TorrentState::Seeding => {
                    if goal.seeding_since.is_none() {
                        goal.seeding_since = Some(Instant::now());
                    }
                }
                TorrentState::Downloading
                | TorrentState::Queued
                | TorrentState::FetchingMetadata
                | TorrentState::Stopped
                | TorrentState::Failed { .. } => {
                    goal.seeding_since = None;
                }
            }
        }
    }

    fn evaluate_seeding_goal(
        &mut self,
        torrent_id: Uuid,
        ratio: f64,
        actions: &mut Vec<PendingAction>,
    ) {
        if let Some(goal) = self.seeding_goals.get_mut(&torrent_id) {
            if goal.satisfied || goal.seeding_since.is_none() {
                return;
            }
            let ratio_met = goal
                .ratio_limit
                .is_some_and(|limit| ratio.is_finite() && ratio >= limit);
            let time_met = goal.time_limit.is_some_and(|limit| {
                goal.seeding_since
                    .is_some_and(|since| since.elapsed() >= limit)
            });

            if ratio_met || time_met {
                goal.satisfied = true;
                actions.push(PendingAction::PauseForSeedingGoal(torrent_id));
            }
        }
    }

    fn enqueue_time_based_goals(&mut self, actions: &mut Vec<PendingAction>) {
        let now = Instant::now();
        for (id, goal) in &mut self.seeding_goals {
            if goal.satisfied {
                continue;
            }
            if let (Some(limit), Some(started)) = (goal.time_limit, goal.seeding_since)
                && now.duration_since(started) >= limit
            {
                goal.satisfied = true;
                actions.push(PendingAction::PauseForSeedingGoal(*id));
            }
        }
    }

    async fn apply_actions(&mut self, actions: Vec<PendingAction>) -> Result<()> {
        let mut seen = HashSet::new();
        for action in actions {
            match action {
                PendingAction::PauseForSeedingGoal(id) => {
                    if !seen.insert(id) {
                        continue;
                    }
                    self.session.pause_torrent(id).await?;
                    info!(torrent_id = %id, "seeding goal reached; pausing torrent");
                }
            }
        }
        Ok(())
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

fn alt_speed_plan(config: AltSpeedRuntimeConfig) -> Option<AltSpeedPlan> {
    let limits = TorrentRateLimit {
        download_bps: map_limit(config.download_bps),
        upload_bps: map_limit(config.upload_bps),
    };
    if limits.download_bps.is_none() && limits.upload_bps.is_none() {
        return None;
    }

    Some(AltSpeedPlan {
        limits,
        schedule: config.schedule,
        active: false,
    })
}

fn is_alt_speed_active(schedule: &AltSpeedSchedule, now: DateTime<Utc>) -> bool {
    if !schedule.days.contains(&now.weekday()) {
        return false;
    }
    let minutes_today =
        u16::try_from(now.hour() * 60 + now.minute()).unwrap_or(schedule.start_minutes);

    if schedule.start_minutes < schedule.end_minutes {
        minutes_today >= schedule.start_minutes && minutes_today < schedule.end_minutes
    } else {
        minutes_today >= schedule.start_minutes || minutes_today < schedule.end_minutes
    }
}

const fn has_rate_limit(limit: &TorrentRateLimit) -> bool {
    limit.download_bps.is_some() || limit.upload_bps.is_some()
}

fn rate_limit_for_metadata(limit: &TorrentRateLimit) -> Option<TorrentRateLimit> {
    if has_rate_limit(limit) {
        Some(limit.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChokingAlgorithm, EncryptionPolicy, Ipv6Mode, SeedChokingAlgorithm, TrackerRuntimeConfig,
        command::EngineCommand,
        session::StubSession,
        store::{FastResumeStore, StoredTorrentMetadata},
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use chrono::{TimeZone, Utc, Weekday};
    use revaer_events::{Event, EventBus, TorrentState};
    use revaer_torrent_core::{
        AddTorrent, AddTorrentOptions, FilePriority, FilePriorityOverride, FileSelectionRules,
        FileSelectionUpdate, PeerChoke, PeerInterest, PeerSnapshot, RemoveTorrent, TorrentProgress,
        TorrentRateLimit, TorrentRates, TorrentSource, model::TorrentOptionsUpdate,
    };
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::{
        sync::oneshot,
        time::{sleep, timeout},
    };
    use tokio_stream::StreamExt;

    const SAMPLE_PIECE_HASH: [u8; 20] = [
        137, 114, 86, 182, 112, 158, 26, 77, 169, 218, 186, 146, 182, 189, 227, 156, 207, 204, 216,
        193,
    ];

    async fn next_event_with_timeout(
        stream: &mut revaer_events::EventStream,
        timeout_ms: u64,
    ) -> Option<Event> {
        match timeout(Duration::from_millis(timeout_ms), stream.next()).await {
            Ok(Some(Ok(envelope))) => Some(envelope.event),
            _ => None,
        }
    }

    type DeadlineLog = std::sync::Arc<tokio::sync::Mutex<Vec<(Uuid, u32, Option<u32>)>>>;

    #[derive(Clone, Default)]
    struct DeadlineSession {
        deadlines: DeadlineLog,
    }

    #[async_trait]
    impl LibTorrentSession for DeadlineSession {
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

        async fn update_options(
            &mut self,
            _id: Uuid,
            _options: &revaer_torrent_core::model::TorrentOptionsUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_trackers(
            &mut self,
            _id: Uuid,
            _trackers: &revaer_torrent_core::model::TorrentTrackersUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_web_seeds(
            &mut self,
            _id: Uuid,
            _web_seeds: &revaer_torrent_core::model::TorrentWebSeedsUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn reannounce(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn move_torrent(&mut self, _id: Uuid, _download_dir: &str) -> Result<()> {
            Ok(())
        }

        async fn recheck(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn peers(&mut self, _id: Uuid) -> Result<Vec<PeerSnapshot>> {
            Ok(Vec::new())
        }

        async fn poll_events(&mut self) -> Result<Vec<EngineEvent>> {
            Ok(Vec::new())
        }

        async fn apply_config(&mut self, _config: &EngineRuntimeConfig) -> Result<()> {
            Ok(())
        }

        async fn set_piece_deadline(
            &mut self,
            id: Uuid,
            piece: u32,
            deadline_ms: Option<u32>,
        ) -> Result<()> {
            self.deadlines.lock().await.push((id, piece, deadline_ms));
            Ok(())
        }
    }

    fn sample_metainfo() -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(
            b"d8:announce30:http://localhost:6969/announce4:infod6:lengthi16384e4:name6:sample12:piece lengthi16384e6:pieces20:",
        );
        encoded.extend_from_slice(&SAMPLE_PIECE_HASH);
        encoded.extend_from_slice(b"ee");
        encoded
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
                auto_managed: Some(false),
                queue_position: Some(3),
                pex_enabled: Some(false),
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor.clone())))
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

        let metadata = worker
            .resume_cache
            .get(&descriptor.id)
            .expect("metadata stored");
        assert_eq!(metadata.auto_managed, Some(false));
        assert_eq!(metadata.queue_position, Some(3));
        assert_eq!(metadata.pex_enabled, Some(false));

        Ok(())
    }

    #[tokio::test]
    async fn add_command_applies_per_torrent_rate_limit() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus, session, None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:ratelimit"),
            options: AddTorrentOptions {
                rate_limit: TorrentRateLimit {
                    download_bps: Some(12_000),
                    upload_bps: Some(6_000),
                },
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");

        let cached = worker
            .resume_cache
            .get(&torrent_id)
            .cloned()
            .expect("metadata persisted");
        let limit = cached.rate_limit.expect("rate limit persisted to metadata");
        assert_eq!(limit.download_bps, Some(12_000));
        assert_eq!(limit.upload_bps, Some(6_000));
        assert_eq!(
            worker
                .per_torrent_limits
                .get(&torrent_id)
                .and_then(|rate| rate.download_bps),
            Some(12_000)
        );
        Ok(())
    }

    #[tokio::test]
    async fn query_peers_returns_stubbed_snapshot() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let torrent_id = Uuid::new_v4();
        let peer = PeerSnapshot {
            endpoint: "203.0.113.2:6881".to_string(),
            client: Some("lt/peer".to_string()),
            progress: 0.25,
            download_bps: 512,
            upload_bps: 256,
            interest: PeerInterest {
                local: true,
                remote: true,
            },
            choke: PeerChoke {
                local: false,
                remote: false,
            },
        };
        let session: Box<dyn LibTorrentSession> =
            Box::new(StubSession::default().with_peers(torrent_id, vec![peer.clone()]));
        let mut worker = Worker::new(bus, session, None);

        let (respond_to, rx) = oneshot::channel();
        worker
            .handle(EngineCommand::QueryPeers {
                id: torrent_id,
                respond_to,
            })
            .await?;

        let peers = rx.await.expect("response channel")?;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, peer.endpoint);
        assert_eq!(peers[0].download_bps, peer.download_bps);
        Ok(())
    }

    #[tokio::test]
    async fn add_command_persists_connection_limit_metadata() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus, session, None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:limit"),
            options: AddTorrentOptions {
                connections_limit: Some(24),
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");

        let persisted = worker
            .resume_cache
            .get(&torrent_id)
            .and_then(|meta| meta.connections_limit);
        assert_eq!(persisted, Some(24));
        Ok(())
    }

    #[tokio::test]
    async fn update_options_adjusts_metadata_and_goals() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus, session, None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:update-options"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");

        let update = TorrentOptionsUpdate {
            connections_limit: Some(16),
            pex_enabled: Some(false),
            paused: None,
            super_seeding: Some(true),
            auto_managed: Some(false),
            queue_position: Some(3),
            seed_ratio_limit: Some(1.5),
            seed_time_limit: Some(900),
        };
        worker
            .handle(EngineCommand::UpdateOptions {
                id: torrent_id,
                options: update,
            })
            .await?;

        let metadata = worker
            .resume_cache
            .get(&torrent_id)
            .cloned()
            .expect("metadata persisted");
        assert_eq!(metadata.connections_limit, Some(16));
        assert_eq!(metadata.pex_enabled, Some(false));
        assert_eq!(metadata.super_seeding, Some(true));
        assert_eq!(metadata.auto_managed, Some(false));
        assert_eq!(metadata.queue_position, Some(3));
        assert_eq!(metadata.seed_ratio_limit, Some(1.5));
        assert_eq!(metadata.seed_time_limit, Some(900));

        let goal = worker
            .seeding_goals
            .get(&torrent_id)
            .expect("goal recorded");
        assert_eq!(goal.ratio_limit, Some(1.5));
        assert_eq!(goal.time_limit, Some(Duration::from_secs(900)));
        Ok(())
    }

    #[tokio::test]
    async fn update_options_can_toggle_paused_state() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:options-pause"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");
        let _ = next_event_with_timeout(&mut stream, 50).await;
        let _ = next_event_with_timeout(&mut stream, 50).await;

        worker
            .handle(EngineCommand::UpdateOptions {
                id: torrent_id,
                options: TorrentOptionsUpdate {
                    paused: Some(true),
                    ..TorrentOptionsUpdate::default()
                },
            })
            .await?;

        let mut paused_event = None;
        for _ in 0..5 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::StateChanged {
                    torrent_id: event_id,
                    state,
                }) => {
                    paused_event = Some((event_id, state));
                    break;
                }
                Some(Event::MetadataUpdated { .. }) => {}
                other => panic!("expected paused state change, got {other:?}"),
            }
        }
        let Some((event_id, state)) = paused_event else {
            panic!("state change not observed after pause");
        };
        assert_eq!(event_id, torrent_id);
        assert!(matches!(state, TorrentState::Stopped));

        worker
            .handle(EngineCommand::UpdateOptions {
                id: torrent_id,
                options: TorrentOptionsUpdate {
                    paused: Some(false),
                    ..TorrentOptionsUpdate::default()
                },
            })
            .await?;

        let mut resumed_event = None;
        for _ in 0..5 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::StateChanged {
                    torrent_id: event_id,
                    state,
                }) => {
                    resumed_event = Some((event_id, state));
                    break;
                }
                Some(Event::MetadataUpdated { .. }) => {}
                other => panic!("expected resumed state change, got {other:?}"),
            }
        }
        let Some((event_id, state)) = resumed_event else {
            panic!("state change not observed after resume");
        };
        assert_eq!(event_id, torrent_id);
        assert!(matches!(state, TorrentState::Downloading));

        Ok(())
    }

    #[tokio::test]
    async fn piece_deadline_command_invokes_session() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let session = DeadlineSession::default();
        let log = session.deadlines.clone();
        let mut worker = Worker::new(bus, Box::new(session), None);
        let torrent_id = Uuid::new_v4();

        worker
            .handle(EngineCommand::SetPieceDeadline {
                id: torrent_id,
                piece: 3,
                deadline_ms: Some(1_200),
            })
            .await?;

        let entries = log.lock().await.clone();
        assert_eq!(entries, vec![(torrent_id, 3, Some(1_200))]);
        Ok(())
    }

    #[tokio::test]
    async fn update_trackers_and_web_seeds_persist_metadata() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus, session, None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet("magnet:?xt=urn:btih:update-trackers"),
            options: AddTorrentOptions::default(),
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");

        worker
            .handle(EngineCommand::UpdateTrackers {
                id: torrent_id,
                trackers: TorrentTrackersUpdate {
                    trackers: vec!["https://tracker.example/announce".to_string()],
                    replace: true,
                },
            })
            .await?;

        worker
            .handle(EngineCommand::UpdateWebSeeds {
                id: torrent_id,
                web_seeds: TorrentWebSeedsUpdate {
                    web_seeds: vec!["http://seed.example/file".to_string()],
                    replace: false,
                },
            })
            .await?;

        let metadata = worker
            .resume_cache
            .get(&torrent_id)
            .cloned()
            .expect("metadata persisted");
        assert_eq!(
            metadata.trackers,
            vec!["https://tracker.example/announce".to_string()]
        );
        assert!(metadata.replace_trackers);
        assert_eq!(
            metadata.web_seeds,
            vec!["http://seed.example/file".to_string()]
        );
        assert!(!metadata.replace_web_seeds);
        Ok(())
    }

    #[tokio::test]
    async fn move_command_updates_metadata_and_emits_event() -> Result<()> {
        let bus = EventBus::with_capacity(16);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet("magnet:?xt=urn:btih:move"),
            options: AddTorrentOptions {
                download_dir: Some("/downloads/original".into()),
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor.clone())))
            .await?;
        while next_event_with_timeout(&mut stream, 10).await.is_some() {}

        worker
            .handle(EngineCommand::MoveStorage {
                id: descriptor.id,
                download_dir: "/downloads/relocated".into(),
            })
            .await?;

        let mut metadata_event_seen = false;
        for _ in 0..3 {
            if let Some(Event::MetadataUpdated {
                torrent_id,
                download_dir,
                ..
            }) = next_event_with_timeout(&mut stream, 50).await
            {
                assert_eq!(torrent_id, descriptor.id);
                assert_eq!(download_dir.as_deref(), Some("/downloads/relocated"));
                metadata_event_seen = true;
                break;
            }
        }

        assert!(metadata_event_seen, "metadata update should be emitted");

        let cached_dir = worker
            .resume_cache
            .get(&descriptor.id)
            .and_then(|meta| meta.download_dir.clone());
        assert_eq!(cached_dir.as_deref(), Some("/downloads/relocated"));

        Ok(())
    }

    #[tokio::test]
    async fn add_command_records_seed_mode_and_sample() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);

        let torrent_id = Uuid::new_v4();
        let descriptor = AddTorrent {
            id: torrent_id,
            source: TorrentSource::metainfo(sample_metainfo()),
            options: AddTorrentOptions {
                seed_mode: Some(true),
                hash_check_sample_pct: Some(5),
                ..AddTorrentOptions::default()
            },
        };

        worker
            .handle(EngineCommand::Add(Box::new(descriptor)))
            .await
            .expect("add should succeed");

        let mut saw_seed_state = false;
        for _ in 0..3 {
            if let Some(Event::StateChanged {
                torrent_id: event_id,
                state,
            }) = next_event_with_timeout(&mut stream, 50).await
                && event_id == torrent_id
                && matches!(state, TorrentState::Seeding)
            {
                saw_seed_state = true;
                break;
            }
        }
        assert!(saw_seed_state, "expected seeding state change");

        let persisted = worker
            .resume_cache
            .get(&torrent_id)
            .cloned()
            .expect("metadata persisted");
        assert_eq!(persisted.seed_mode, Some(true));
        assert_eq!(persisted.hash_check_sample_pct, Some(5));
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
            .handle(EngineCommand::Add(Box::new(descriptor.clone())))
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

        let mut observed = None;
        for _ in 0..5 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::StateChanged { torrent_id, state }) => {
                    observed = Some((torrent_id, state));
                    break;
                }
                Some(Event::MetadataUpdated { .. }) => {}
                other => panic!("expected stopped state change, got {other:?}"),
            }
        }
        let Some((torrent_id, state)) = observed else {
            panic!("state change not observed after remove");
        };
        assert_eq!(torrent_id, descriptor.id);
        assert!(matches!(state, TorrentState::Stopped));

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
            .handle(EngineCommand::Add(Box::new(descriptor.clone())))
            .await?;
        let _ = next_event_with_timeout(&mut stream, 50).await;
        let _ = next_event_with_timeout(&mut stream, 50).await;

        worker
            .handle(EngineCommand::Pause { id: descriptor.id })
            .await?;
        let mut paused_event = None;
        for _ in 0..5 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::StateChanged { torrent_id, state }) => {
                    paused_event = Some((torrent_id, state));
                    break;
                }
                Some(Event::MetadataUpdated { .. }) => {}
                other => panic!("expected stopped state, got {other:?}"),
            }
        }
        let Some((torrent_id, state)) = paused_event else {
            panic!("state change not observed after pause");
        };
        assert_eq!(torrent_id, descriptor.id);
        assert!(matches!(state, TorrentState::Stopped));

        worker
            .handle(EngineCommand::Resume { id: descriptor.id })
            .await?;
        let mut resumed_event = None;
        for _ in 0..5 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::StateChanged { torrent_id, state }) => {
                    resumed_event = Some((torrent_id, state));
                    break;
                }
                Some(Event::MetadataUpdated { .. }) => {}
                other => panic!("expected downloading state, got {other:?}"),
            }
        }
        let Some((torrent_id, state)) = resumed_event else {
            panic!("state change not observed after resume");
        };
        assert_eq!(torrent_id, descriptor.id);
        assert!(matches!(state, TorrentState::Downloading));

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
            storage_mode: Some(StorageMode::Sparse),
            use_partfile: Some(true),
            sequential: true,
            trackers: vec!["https://tracker.example/announce".into()],
            replace_trackers: true,
            tracker_messages: HashMap::new(),
            web_seeds: vec!["https://seed.example/file".into()],
            replace_web_seeds: false,
            tags: vec!["persisted".into()],
            rate_limit: Some(TorrentRateLimit {
                download_bps: Some(5_000),
                upload_bps: Some(2_500),
            }),
            connections_limit: None,
            seed_mode: None,
            hash_check_sample_pct: None,
            super_seeding: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            auto_managed: Some(true),
            queue_position: Some(1),
            pex_enabled: Some(true),
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
            .handle(EngineCommand::Add(Box::new(descriptor.clone())))
            .await?;

        let mut reconciled = false;
        for _ in 0..8 {
            match next_event_with_timeout(&mut stream, 50).await {
                Some(Event::SelectionReconciled { torrent_id, .. }) => {
                    assert_eq!(torrent_id, descriptor.id);
                    reconciled = true;
                    break;
                }
                Some(_) => {}
                None => break,
            }
        }
        assert!(reconciled, "expected selection reconciliation event");

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
        assert_eq!(
            persisted_meta.web_seeds,
            vec!["https://seed.example/file".to_string()]
        );
        assert!(!persisted_meta.replace_web_seeds);
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

        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::Progress {
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
            },
            &mut actions,
        );
        worker.apply_actions(actions).await?;

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::Progress { torrent_id: id, .. }) => assert_eq!(id, torrent_id),
            other => panic!("expected progress event, got {other:?}"),
        }

        // Immediate second update should be suppressed to honour the coalescing budget.
        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::Progress {
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
            },
            &mut actions,
        );
        worker.apply_actions(actions).await?;

        assert!(
            next_event_with_timeout(&mut stream, 20).await.is_none(),
            "second event should have been coalesced"
        );

        // After the coalescing interval elapses the next event should be emitted.
        sleep(PROGRESS_COALESCE_INTERVAL).await;
        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::Progress {
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
            },
            &mut actions,
        );
        worker.apply_actions(actions).await?;

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

        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::Progress {
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
            },
            &mut actions,
        );
        worker.apply_actions(actions).await?;

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
        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::Progress {
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
            },
            &mut actions,
        );
        worker.apply_actions(actions).await?;

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert!(degraded.is_empty(), "expected recovery event");
            }
            other => panic!("expected health recovery event, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn session_error_event_degrades_health() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);
        let mut actions = Vec::new();

        worker.publish_engine_event(
            EngineEvent::SessionError {
                component: Some("network".to_string()),
                message: "bind failed".to_string(),
            },
            &mut actions,
        );

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert_eq!(degraded, vec!["network".to_string()]);
            }
            other => panic!("expected health change event, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn tracker_error_marks_degraded_and_recovers() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let mut stream = bus.subscribe(None);
        let torrent_id = Uuid::new_v4();
        let mut actions = Vec::new();

        worker.publish_engine_event(
            EngineEvent::TrackerStatus {
                torrent_id,
                trackers: vec![TrackerStatus {
                    url: "https://tracker.example/announce".to_string(),
                    status: Some("error".to_string()),
                    message: Some("unreachable".to_string()),
                }],
            },
            &mut actions,
        );

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert_eq!(degraded, vec!["tracker".to_string()]);
            }
            other => panic!("expected tracker degradation event, got {other:?}"),
        }

        let mut actions = Vec::new();
        worker.publish_engine_event(
            EngineEvent::TrackerStatus {
                torrent_id,
                trackers: vec![TrackerStatus {
                    url: "https://tracker.example/announce".to_string(),
                    status: None,
                    message: None,
                }],
            },
            &mut actions,
        );

        match next_event_with_timeout(&mut stream, 50).await {
            Some(Event::HealthChanged { degraded }) => {
                assert!(degraded.is_empty(), "expected tracker recovery");
            }
            other => panic!("expected tracker recovery event, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn alt_speed_schedule_applies_and_reverts() -> Result<()> {
        let bus = EventBus::with_capacity(4);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus, session, None);

        let schedule = AltSpeedSchedule {
            days: vec![Weekday::Mon],
            start_minutes: 60,
            end_minutes: 180,
        };
        let config = EngineRuntimeConfig {
            download_root: "/data".into(),
            resume_dir: "/state".into(),
            storage_mode: StorageMode::Sparse.into(),
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
            listen_port: None,
            max_active: None,
            download_rate_limit: Some(100_000),
            upload_rate_limit: Some(50_000),
            seed_ratio_limit: None,
            seed_time_limit: None,
            alt_speed: Some(AltSpeedRuntimeConfig {
                download_bps: Some(10_000),
                upload_bps: None,
                schedule: schedule.clone(),
            }),
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
            super_seeding: false.into(),
        };

        worker
            .handle(EngineCommand::ApplyConfig(Box::new(config)))
            .await?;

        let active_monday = Utc
            .with_ymd_and_hms(2024, 1, 1, 1, 30, 0)
            .single()
            .expect("valid datetime");
        worker.reconcile_alt_speed_with_now(active_monday).await?;
        assert_eq!(worker.global_limits.download_bps, Some(10_000));
        assert!(worker.alt_speed.as_ref().is_some_and(|plan| plan.active));

        let inactive_tuesday = Utc
            .with_ymd_and_hms(2024, 1, 2, 3, 0, 0)
            .single()
            .expect("valid datetime");
        worker
            .reconcile_alt_speed_with_now(inactive_tuesday)
            .await?;
        assert_eq!(worker.global_limits.download_bps, Some(100_000));
        assert!(!worker.alt_speed.as_ref().is_some_and(|plan| plan.active));
        Ok(())
    }

    #[test]
    fn alt_speed_schedule_handles_wraparound() {
        let schedule = AltSpeedSchedule {
            days: vec![Weekday::Mon],
            start_minutes: 22 * 60,
            end_minutes: 3 * 60,
        };

        let late_monday = Utc
            .with_ymd_and_hms(2024, 1, 1, 23, 0, 0)
            .single()
            .expect("valid datetime");
        assert!(is_alt_speed_active(&schedule, late_monday));

        let early_monday = Utc
            .with_ymd_and_hms(2024, 1, 1, 1, 0, 0)
            .single()
            .expect("valid datetime");
        assert!(is_alt_speed_active(&schedule, early_monday));

        let tuesday = Utc
            .with_ymd_and_hms(2024, 1, 2, 10, 0, 0)
            .single()
            .expect("valid datetime");
        assert!(!is_alt_speed_active(&schedule, tuesday));
    }

    #[tokio::test]
    async fn seeding_goals_pause_when_ratio_exceeded() -> Result<()> {
        let bus = EventBus::with_capacity(8);
        let session: Box<dyn LibTorrentSession> = Box::new(StubSession::default());
        let mut worker = Worker::new(bus.clone(), session, None);
        let torrent_id = Uuid::new_v4();
        let mut stream = bus.subscribe(None);

        let request = AddTorrent {
            id: torrent_id,
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567",
            ),
            options: AddTorrentOptions {
                seed_ratio_limit: Some(0.8),
                ..AddTorrentOptions::default()
            },
        };
        worker.handle_add(request).await?;

        // Drain initial events from the add flow.
        while next_event_with_timeout(&mut stream, 10).await.is_some() {}

        worker.update_seeding_state(torrent_id, &TorrentState::Completed);
        let mut actions = Vec::new();
        worker.evaluate_seeding_goal(torrent_id, 0.85, &mut actions);
        worker.apply_actions(actions).await?;
        worker.flush_session_events().await?;

        let mut paused = false;
        while let Some(event) = next_event_with_timeout(&mut stream, 20).await {
            if let Event::StateChanged {
                torrent_id: id,
                state: TorrentState::Stopped,
            } = event
                && id == torrent_id
            {
                paused = true;
                break;
            }
        }
        assert!(paused, "torrent should pause after hitting seeding goal");
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

        async fn update_options(
            &mut self,
            _id: Uuid,
            _options: &revaer_torrent_core::model::TorrentOptionsUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_trackers(
            &mut self,
            _id: Uuid,
            _trackers: &revaer_torrent_core::model::TorrentTrackersUpdate,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_web_seeds(
            &mut self,
            _id: Uuid,
            _web_seeds: &revaer_torrent_core::model::TorrentWebSeedsUpdate,
        ) -> Result<()> {
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

        async fn move_torrent(&mut self, _id: Uuid, _download_dir: &str) -> Result<()> {
            Ok(())
        }

        async fn recheck(&mut self, _id: Uuid) -> Result<()> {
            Ok(())
        }

        async fn peers(&mut self, _id: Uuid) -> Result<Vec<PeerSnapshot>> {
            Err(anyhow::anyhow!("simulated session failure"))
        }

        async fn set_piece_deadline(
            &mut self,
            _id: Uuid,
            _piece: u32,
            _deadline_ms: Option<u32>,
        ) -> Result<()> {
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
