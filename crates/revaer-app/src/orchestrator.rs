//! Torrent orchestrator that wires libtorrent engine events into filesystem
//! post-processing and runtime persistence.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine_config::EngineRuntimePlan;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, header::IF_NONE_MATCH};
use revaer_config::engine_profile::canonicalize_ip_filter_entry;
use revaer_config::{
    ConfigService, EngineProfile, FsPolicy, IpFilterConfig, IpFilterRule, SettingsChangeset,
    SettingsFacade,
};
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_fsops::{FsOpsRequest, FsOpsService};
use revaer_runtime::RuntimeStore;
use revaer_telemetry::Metrics;
use revaer_torrent_core::{
    AddTorrent, FilePriority, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentFile,
    TorrentInspector, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentStatus,
    TorrentWorkflow,
    model::{TorrentOptionsUpdate, TorrentTrackersUpdate, TorrentWebSeedsUpdate},
};
use revaer_torrent_libt::{IpFilterRule as RuntimeIpFilterRule, IpFilterRuntimeConfig};
use serde_json::json;
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use uuid::Uuid;

#[async_trait]
pub(crate) trait EngineConfigurator: Send + Sync {
    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> Result<()>;
}

const BLOCKLIST_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
const MAX_BLOCKLIST_RULES: usize = 100_000;

#[derive(Clone)]
struct IpFilterCache {
    url: String,
    etag: Option<String>,
    rules: Vec<RuntimeIpFilterRule>,
    fetched_at: Instant,
    last_refreshed: DateTime<Utc>,
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
    config: Option<ConfigService>,
    http: Client,
    ip_filter_cache: RwLock<Option<IpFilterCache>>,
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
        config: Option<ConfigService>,
    ) -> Self {
        Self {
            engine,
            fsops,
            events,
            fs_policy: Arc::new(RwLock::new(fs_policy)),
            engine_profile: Arc::new(RwLock::new(engine_profile)),
            catalog: Arc::new(TorrentCatalog::new()),
            runtime,
            config,
            http: Client::new(),
            ip_filter_cache: RwLock::new(None),
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
        let mut plan = EngineRuntimePlan::from_profile(&profile);
        self.refresh_ip_filter(&mut plan).await?;
        self.resolve_tracker_auth(&mut plan).await?;
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

    async fn refresh_ip_filter(&self, plan: &mut EngineRuntimePlan) -> Result<()> {
        let previous = plan.effective.network.ip_filter.clone();
        let mut runtime_filter =
            plan.runtime
                .ip_filter
                .clone()
                .unwrap_or_else(|| IpFilterRuntimeConfig {
                    rules: Vec::new(),
                    blocklist_url: None,
                    etag: None,
                    last_updated_at: None,
                });

        let mut rules = runtime_filter.rules.clone();
        let mut etag = previous.etag.clone();
        let mut last_updated_at = previous.last_updated_at;
        let mut warnings = Vec::new();
        let mut last_error: Option<String> = None;

        let cached_filter = self.ip_filter_cache.read().await.clone();
        if let Some(cache) = cached_filter
            && Some(cache.url.as_str()) != previous.blocklist_url.as_deref()
        {
            self.clear_ip_filter_cache().await;
        }

        runtime_filter
            .blocklist_url
            .clone_from(&previous.blocklist_url);

        if let Some(url) = previous.blocklist_url.clone() {
            match self.load_blocklist(&url, etag.clone()).await {
                Ok(resolution) => {
                    merge_rules(&mut rules, resolution.rules);
                    etag = resolution.etag;
                    last_updated_at = Some(resolution.last_updated_at);
                    if resolution.skipped > 0 {
                        warnings.push(format!(
                            "skipped {} invalid blocklist entries from {}",
                            resolution.skipped, url
                        ));
                    }
                    runtime_filter.etag.clone_from(&etag);
                    runtime_filter.last_updated_at =
                        last_updated_at.map(|timestamp| timestamp.to_rfc3339());
                }
                Err(err) => {
                    let message = format!("blocklist fetch failed for {url}: {err}");
                    warnings.push(message.clone());
                    last_error = Some(message);
                    runtime_filter.etag.clone_from(&etag);
                    runtime_filter.last_updated_at =
                        last_updated_at.map(|timestamp| timestamp.to_rfc3339());
                }
            }
        } else {
            self.clear_ip_filter_cache().await;
            runtime_filter.etag = None;
            runtime_filter.last_updated_at = None;
        }

        runtime_filter.rules = dedupe_rules(rules);
        plan.runtime.ip_filter = Some(runtime_filter);

        plan.effective.network.ip_filter.etag.clone_from(&etag);
        plan.effective.network.ip_filter.last_updated_at = last_updated_at;
        plan.effective
            .network
            .ip_filter
            .last_error
            .clone_from(&last_error);
        plan.effective.warnings.extend(warnings);

        if let Some(config) = &self.config
            && let Err(err) = self
                .persist_ip_filter_metadata(config, &previous, &plan.effective.network.ip_filter)
                .await
        {
            warn!(
                error = %err,
                "failed to persist ip_filter metadata; continuing with cached state"
            );
            plan.effective.warnings.push(
                "failed to persist ip_filter metadata; continuing with cached state".to_string(),
            );
        }

        Ok(())
    }

    async fn resolve_tracker_auth(&self, plan: &mut EngineRuntimePlan) -> Result<()> {
        let Some(config) = &self.config else {
            return Ok(());
        };
        let Some(auth) = plan.runtime.tracker.auth.as_mut() else {
            return Ok(());
        };

        let mut warnings = Vec::new();

        if auth.username.is_none()
            && let Some(secret) = &auth.username_secret
        {
            match config.get_secret(secret).await? {
                Some(value) => auth.username = Some(value),
                None => warnings.push(format!(
                    "tracker.auth.username_secret '{secret}' is not set"
                )),
            }
        }
        if auth.password.is_none()
            && let Some(secret) = &auth.password_secret
        {
            match config.get_secret(secret).await? {
                Some(value) => auth.password = Some(value),
                None => warnings.push(format!(
                    "tracker.auth.password_secret '{secret}' is not set"
                )),
            }
        }
        if auth.cookie.is_none()
            && let Some(secret) = &auth.cookie_secret
        {
            match config.get_secret(secret).await? {
                Some(value) => auth.cookie = Some(value),
                None => warnings.push(format!("tracker.auth.cookie_secret '{secret}' is not set")),
            }
        }

        if !warnings.is_empty() {
            plan.effective.warnings.extend(warnings);
        }

        Ok(())
    }

    async fn load_blocklist(&self, url: &str, etag: Option<String>) -> Result<BlocklistResolution> {
        let cached = self.ip_filter_cache.read().await.clone();
        if let Some(cache) = cached.as_ref()
            && cache.url == url
            && cache.fetched_at.elapsed() < BLOCKLIST_REFRESH_INTERVAL
        {
            return Ok(BlocklistResolution {
                rules: cache.rules.clone(),
                etag: cache.etag.clone(),
                last_updated_at: cache.last_refreshed,
                skipped: 0,
            });
        }

        let mut request = self.http.get(url);
        if let Some(tag) = etag
            .clone()
            .or_else(|| cached.as_ref().and_then(|c| c.etag.clone()))
        {
            request = request.header(IF_NONE_MATCH, tag);
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(cache) = cached {
                let now = Utc::now();
                self.set_ip_filter_cache(IpFilterCache {
                    url: cache.url,
                    etag: cache.etag.clone(),
                    rules: cache.rules.clone(),
                    fetched_at: Instant::now(),
                    last_refreshed: now,
                })
                .await;
                return Ok(BlocklistResolution {
                    rules: cache.rules,
                    etag: cache.etag,
                    last_updated_at: now,
                    skipped: 0,
                });
            }
            return Err(anyhow!("blocklist returned 304 without cached rules"));
        }

        if !response.status().is_success() {
            return Err(anyhow!(
                "blocklist fetch failed with status {}",
                response.status()
            ));
        }

        let etag_header = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response.text().await?;
        let parsed = parse_blocklist(&body)?;
        let now = Utc::now();
        let merged_etag = etag_header.clone().or(etag);
        self.set_ip_filter_cache(IpFilterCache {
            url: url.to_string(),
            etag: merged_etag.clone(),
            rules: parsed.rules.clone(),
            fetched_at: Instant::now(),
            last_refreshed: now,
        })
        .await;

        Ok(BlocklistResolution {
            rules: parsed.rules,
            etag: merged_etag,
            last_updated_at: now,
            skipped: parsed.skipped,
        })
    }

    async fn set_ip_filter_cache(&self, cache: IpFilterCache) {
        let mut guard = self.ip_filter_cache.write().await;
        *guard = Some(cache);
    }

    async fn clear_ip_filter_cache(&self) {
        let mut guard = self.ip_filter_cache.write().await;
        *guard = None;
    }

    async fn persist_ip_filter_metadata(
        &self,
        config: &ConfigService,
        previous: &IpFilterConfig,
        updated: &IpFilterConfig,
    ) -> Result<()> {
        if previous.etag == updated.etag
            && previous.last_updated_at == updated.last_updated_at
            && previous.last_error == updated.last_error
        {
            return Ok(());
        }

        let patch = json!({
            "ip_filter": {
                "cidrs": updated.cidrs,
                "blocklist_url": updated.blocklist_url,
                "etag": updated.etag,
                "last_updated_at": updated.last_updated_at.map(|ts| ts.to_rfc3339()),
                "last_error": updated.last_error,
            }
        });
        let changeset = SettingsChangeset {
            engine_profile: Some(patch),
            ..SettingsChangeset::default()
        };
        config
            .apply_changeset("system", "ip_filter_refresh", changeset)
            .await?;
        Ok(())
    }

    /// Remove the torrent from the engine.
    pub(crate) async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> Result<()> {
        self.engine.remove_torrent(id, options).await
    }
}

struct BlocklistResolution {
    rules: Vec<RuntimeIpFilterRule>,
    etag: Option<String>,
    last_updated_at: DateTime<Utc>,
    skipped: usize,
}

struct ParsedBlocklist {
    rules: Vec<RuntimeIpFilterRule>,
    skipped: usize,
}

fn parse_blocklist(body: &str) -> Result<ParsedBlocklist> {
    let mut rules = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped = 0usize;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || trimmed.starts_with(';')
        {
            continue;
        }

        match canonicalize_ip_filter_entry(trimmed, "ip_filter.blocklist_url") {
            Ok((canonical, rule)) => {
                if seen.insert(canonical.to_ascii_lowercase()) {
                    rules.push(runtime_rule_from_config(&rule));
                    if rules.len() > MAX_BLOCKLIST_RULES {
                        return Err(anyhow!(
                            "blocklist contains more than {MAX_BLOCKLIST_RULES} entries"
                        ));
                    }
                }
            }
            Err(_) => {
                skipped = skipped.saturating_add(1);
            }
        }
    }

    Ok(ParsedBlocklist { rules, skipped })
}

fn runtime_rule_from_config(rule: &IpFilterRule) -> RuntimeIpFilterRule {
    RuntimeIpFilterRule {
        start: rule.start.to_string(),
        end: rule.end.to_string(),
    }
}

fn merge_rules(base: &mut Vec<RuntimeIpFilterRule>, additions: Vec<RuntimeIpFilterRule>) {
    let mut seen: HashSet<String> = base
        .iter()
        .map(|rule| format!("{}-{}", rule.start, rule.end).to_ascii_lowercase())
        .collect();

    for rule in additions {
        let key = format!("{}-{}", rule.start, rule.end).to_ascii_lowercase();
        if seen.insert(key) {
            base.push(rule);
        }
    }
}

fn dedupe_rules(rules: Vec<RuntimeIpFilterRule>) -> Vec<RuntimeIpFilterRule> {
    let mut deduped = Vec::new();
    merge_rules(&mut deduped, rules);
    deduped
}

const fn event_torrent_id(event: &Event) -> Option<Uuid> {
    match event {
        Event::TorrentAdded { torrent_id, .. }
        | Event::FilesDiscovered { torrent_id, .. }
        | Event::Progress { torrent_id, .. }
        | Event::StateChanged { torrent_id, .. }
        | Event::Completed { torrent_id, .. }
        | Event::MetadataUpdated { torrent_id, .. }
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
    config: Option<ConfigService>,
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
        config,
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

    async fn update_options(&self, id: Uuid, options: TorrentOptionsUpdate) -> anyhow::Result<()> {
        self.engine.update_options(id, options).await
    }

    async fn update_trackers(
        &self,
        id: Uuid,
        trackers: TorrentTrackersUpdate,
    ) -> anyhow::Result<()> {
        self.engine.update_trackers(id, trackers).await
    }

    async fn update_web_seeds(
        &self,
        id: Uuid,
        web_seeds: TorrentWebSeedsUpdate,
    ) -> anyhow::Result<()> {
        self.engine.update_web_seeds(id, web_seeds).await
    }

    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        self.engine.reannounce(id).await
    }

    async fn move_torrent(&self, id: Uuid, download_dir: String) -> anyhow::Result<()> {
        self.engine.move_torrent(id, download_dir).await
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
            Event::MetadataUpdated {
                torrent_id,
                name,
                download_dir,
            } => {
                Self::record_metadata(
                    entries,
                    *torrent_id,
                    name.as_deref(),
                    download_dir.as_deref(),
                );
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

    fn record_metadata(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        name: Option<&str>,
        download_dir: Option<&str>,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        if let Some(name) = name {
            entry.name = Some(name.to_owned());
        }
        if let Some(download_dir) = download_dir {
            entry.download_dir = Some(download_dir.to_owned());
        }
        entry.last_updated = Utc::now();
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
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".to_string(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: json!({}),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: "/tmp/resume".to_string(),
            download_root: "/downloads".to_string(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            tracker: json!([]),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
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
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".to_string(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".to_string(),
            max_active: Some(4),
            max_download_bps: None,
            max_upload_bps: None,
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: json!({}),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: "/tmp/resume".to_string(),
            download_root: "/downloads".to_string(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            tracker: json!([]),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
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
    async fn blocklist_is_fetched_and_cached() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let body = "10.0.0.1/32\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v1\"\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let engine = Arc::new(RecordingEngine::default());
        let bus = EventBus::new();
        let metrics = Metrics::new()?;
        let fsops = FsOpsService::new(bus.clone(), metrics);
        let mut profile = engine_profile("blocklist");
        profile.ip_filter = json!({ "blocklist_url": format!("http://{addr}") });
        let orchestrator = Arc::new(TorrentOrchestrator::new(
            Arc::clone(&engine),
            fsops,
            bus,
            sample_fs_policy(),
            profile.clone(),
            None,
            None,
        ));

        orchestrator
            .update_engine_profile(profile.clone())
            .await
            .expect("blocklist applied");
        let _ = server.await;

        let first_plan = engine
            .applied
            .read()
            .await
            .last()
            .cloned()
            .expect("runtime config applied");
        let filter = first_plan
            .runtime
            .ip_filter
            .as_ref()
            .expect("ip filter present");
        assert_eq!(filter.rules.len(), 1);
        assert_eq!(filter.rules[0].start, "10.0.0.1");

        // Subsequent updates reuse the cached rules even if the server is gone.
        orchestrator
            .update_engine_profile(profile)
            .await
            .expect("cache apply");
        let cached = engine
            .applied
            .read()
            .await
            .last()
            .cloned()
            .expect("cached runtime config");
        let cached_filter = cached.runtime.ip_filter.as_ref().expect("cached filter");
        assert_eq!(cached_filter.rules.len(), 1);
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
            event_torrent_id(&Event::MetadataUpdated {
                torrent_id: id,
                name: None,
                download_dir: Some("/downloads/demo".into())
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
