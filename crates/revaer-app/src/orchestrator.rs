//! Torrent orchestrator that wires libtorrent engine events into filesystem
//! post-processing and runtime persistence.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::engine_config::EngineRuntimePlan;
use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, header::IF_NONE_MATCH};
use revaer_config::engine_profile::canonicalize_ip_filter_entry;
use revaer_config::{
    EngineProfile, FsPolicy, IpFilterConfig, IpFilterRule, SettingsChangeset, SettingsFacade,
};
use revaer_events::{DiscoveredFile, Event, EventBus, TorrentState};
use revaer_fsops::{FsOpsRequest, FsOpsService};
use revaer_runtime::RuntimeStore;
use revaer_telemetry::Metrics;
use revaer_torrent_core::{
    AddTorrent, FilePriority, FileSelectionUpdate, RemoveTorrent, TorrentEngine, TorrentError,
    TorrentFile, TorrentInspector, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentResult,
    TorrentStatus, TorrentWorkflow,
    model::{
        PeerSnapshot, TorrentAuthorRequest, TorrentAuthorResult, TorrentOptionsUpdate,
        TorrentTrackersUpdate, TorrentWebSeedsUpdate,
    },
};
use revaer_torrent_libt::{IpFilterRule as RuntimeIpFilterRule, IpFilterRuntimeConfig};
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use uuid::Uuid;

#[async_trait]
pub(crate) trait EngineConfigurator: Send + Sync {
    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> TorrentResult<()>;
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
    ) -> AppResult<Self> {
        let engine = Arc::new(
            LibtorrentEngine::new(events.clone())
                .map_err(|err| AppError::torrent("libtorrent_engine.new", err))?,
        );
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
    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> TorrentResult<()> {
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
    config: Option<Arc<dyn SettingsFacade>>,
    http: Client,
    ip_filter_cache: RwLock<Option<IpFilterCache>>,
}

#[cfg(any(feature = "libtorrent", test))]
struct ProgressUpdate {
    bytes_downloaded: u64,
    bytes_total: u64,
    eta_seconds: Option<u64>,
    download_bps: u64,
    upload_bps: u64,
    ratio: f64,
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
        config: Option<Arc<dyn SettingsFacade>>,
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
    pub(crate) async fn add_torrent(&self, request: AddTorrent) -> TorrentResult<()> {
        self.engine.add_torrent(request).await
    }

    /// Author a new `.torrent` metainfo payload via the engine.
    pub(crate) async fn create_torrent(
        &self,
        request: TorrentAuthorRequest,
    ) -> TorrentResult<TorrentAuthorResult> {
        self.ensure_authoring_path_allowed(&request.root_path)
            .await
            .map_err(|err| app_error_to_torrent("create_torrent.validate_path", None, err))?;
        self.engine.create_torrent(request).await
    }

    /// Apply the filesystem policy to a completed torrent.
    pub(crate) async fn apply_fsops(&self, torrent_id: Uuid) -> AppResult<()> {
        let policy = self.fs_policy.read().await.clone();
        let snapshot =
            self.catalog
                .get(torrent_id)
                .await
                .ok_or_else(|| AppError::MissingState {
                    field: "torrent_status",
                    value: Some(torrent_id.to_string()),
                })?;
        let source = snapshot
            .library_path
            .as_deref()
            .ok_or_else(|| AppError::MissingState {
                field: "library_path",
                value: Some(torrent_id.to_string()),
            })?;
        let source_path = PathBuf::from(source);
        self.fsops
            .apply(FsOpsRequest {
                torrent_id,
                source_path: &source_path,
                policy: &policy,
            })
            .map_err(|err| AppError::fsops("fsops.apply", err))
    }

    async fn ensure_authoring_path_allowed(&self, root: &str) -> AppResult<()> {
        let policy = self.fs_policy.read().await.clone();
        let root_path = PathBuf::from(root);
        enforce_allow_paths(&root_path, &policy.allow_paths)
    }

    async fn handle_event(&self, event: &Event) -> AppResult<()> {
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

    async fn apply_engine_plan(&self, plan: &EngineRuntimePlan) -> AppResult<()> {
        self.engine
            .apply_engine_plan(plan)
            .await
            .map_err(|err| AppError::torrent("engine.apply_plan", err))
    }

    /// Update the engine profile and propagate changes to the underlying engine.
    pub(crate) async fn update_engine_profile(&self, profile: EngineProfile) -> AppResult<()> {
        {
            let mut guard = self.engine_profile.write().await;
            *guard = profile.clone();
        }
        let mut plan = EngineRuntimePlan::from_profile(&profile);
        self.refresh_ip_filter(&mut plan).await?;
        self.resolve_tracker_auth(&mut plan).await?;
        self.resolve_proxy_auth(&mut plan).await?;
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

        self.apply_engine_plan(&plan).await?;
        self.engine
            .update_limits(None, plan.global_rate_limit())
            .await
            .map_err(|err| AppError::torrent("engine.update_limits", err))?;
        Ok(())
    }

    async fn refresh_ip_filter(&self, plan: &mut EngineRuntimePlan) -> AppResult<()> {
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
                .persist_ip_filter_metadata(
                    config.as_ref(),
                    &previous,
                    &plan.effective.network.ip_filter,
                )
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

    async fn resolve_tracker_auth(&self, plan: &mut EngineRuntimePlan) -> AppResult<()> {
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
            match config
                .get_secret(secret)
                .await
                .map_err(|err| AppError::config("config.get_secret", err))?
            {
                Some(value) => auth.username = Some(value),
                None => warnings.push(format!(
                    "tracker.auth.username_secret '{secret}' is not set"
                )),
            }
        }
        if auth.password.is_none()
            && let Some(secret) = &auth.password_secret
        {
            match config
                .get_secret(secret)
                .await
                .map_err(|err| AppError::config("config.get_secret", err))?
            {
                Some(value) => auth.password = Some(value),
                None => warnings.push(format!(
                    "tracker.auth.password_secret '{secret}' is not set"
                )),
            }
        }
        if auth.cookie.is_none()
            && let Some(secret) = &auth.cookie_secret
        {
            match config
                .get_secret(secret)
                .await
                .map_err(|err| AppError::config("config.get_secret", err))?
            {
                Some(value) => auth.cookie = Some(value),
                None => warnings.push(format!("tracker.auth.cookie_secret '{secret}' is not set")),
            }
        }

        if !warnings.is_empty() {
            plan.effective.warnings.extend(warnings);
        }

        Ok(())
    }

    async fn resolve_proxy_auth(&self, plan: &mut EngineRuntimePlan) -> AppResult<()> {
        let Some(config) = &self.config else {
            return Ok(());
        };
        let Some(proxy) = plan.runtime.tracker.proxy.as_mut() else {
            return Ok(());
        };

        let mut warnings = Vec::new();

        if proxy.username.is_none()
            && let Some(secret) = &proxy.username_secret
        {
            match config
                .get_secret(secret)
                .await
                .map_err(|err| AppError::config("config.get_secret", err))?
            {
                Some(value) => proxy.username = Some(value),
                None => warnings.push(format!(
                    "tracker.proxy.username_secret '{secret}' is not set"
                )),
            }
        }
        if proxy.password.is_none()
            && let Some(secret) = &proxy.password_secret
        {
            match config
                .get_secret(secret)
                .await
                .map_err(|err| AppError::config("config.get_secret", err))?
            {
                Some(value) => proxy.password = Some(value),
                None => warnings.push(format!(
                    "tracker.proxy.password_secret '{secret}' is not set"
                )),
            }
        }

        if !warnings.is_empty() {
            plan.effective.warnings.extend(warnings);
        }

        Ok(())
    }

    async fn load_blocklist(
        &self,
        url: &str,
        etag: Option<String>,
    ) -> AppResult<BlocklistResolution> {
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

        let response = request
            .send()
            .await
            .map_err(|err| AppError::http("blocklist.fetch", url.to_string(), err))?;
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
            return Err(AppError::MissingState {
                field: "blocklist_cache",
                value: Some(url.to_string()),
            });
        }

        if !response.status().is_success() {
            return Err(AppError::HttpStatus {
                operation: "blocklist.fetch",
                url: url.to_string(),
                status: response.status().as_u16(),
            });
        }

        let etag_header = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response
            .text()
            .await
            .map_err(|err| AppError::http("blocklist.read_body", url.to_string(), err))?;
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
        config: &dyn SettingsFacade,
        previous: &IpFilterConfig,
        updated: &IpFilterConfig,
    ) -> AppResult<()> {
        if previous.etag == updated.etag
            && previous.last_updated_at == updated.last_updated_at
            && previous.last_error == updated.last_error
        {
            return Ok(());
        }

        let mut profile = {
            let guard = self.engine_profile.read().await;
            guard.clone()
        };
        profile.ip_filter = updated.clone();
        let changeset = SettingsChangeset {
            engine_profile: Some(profile),
            ..SettingsChangeset::default()
        };
        config
            .apply_changeset("system", "ip_filter_refresh", changeset)
            .await
            .map_err(|err| AppError::config("config.apply_changeset", err))?;
        Ok(())
    }

    /// Remove the torrent from the engine.
    pub(crate) async fn remove_torrent(
        &self,
        id: Uuid,
        options: RemoveTorrent,
    ) -> TorrentResult<()> {
        self.engine.remove_torrent(id, options).await
    }
}

struct BlocklistResolution {
    rules: Vec<RuntimeIpFilterRule>,
    etag: Option<String>,
    last_updated_at: DateTime<Utc>,
    skipped: usize,
}

#[derive(Debug)]
struct ParsedBlocklist {
    rules: Vec<RuntimeIpFilterRule>,
    skipped: usize,
}

fn parse_blocklist(body: &str) -> AppResult<ParsedBlocklist> {
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
                        return Err(AppError::InvalidConfig {
                            field: "ip_filter.blocklist_url",
                            reason: "too_many_entries",
                            value: Some(MAX_BLOCKLIST_RULES.to_string()),
                        });
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
    config: Option<Arc<dyn SettingsFacade>>,
) -> AppResult<(
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
    async fn add_torrent(&self, request: AddTorrent) -> TorrentResult<()> {
        Self::add_torrent(self, request).await
    }

    async fn create_torrent(
        &self,
        request: TorrentAuthorRequest,
    ) -> TorrentResult<TorrentAuthorResult> {
        Self::create_torrent(self, request).await
    }

    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> TorrentResult<()> {
        Self::remove_torrent(self, id, options).await
    }

    async fn pause_torrent(&self, id: Uuid) -> TorrentResult<()> {
        self.engine.pause_torrent(id).await
    }

    async fn resume_torrent(&self, id: Uuid) -> TorrentResult<()> {
        self.engine.resume_torrent(id).await
    }

    async fn set_sequential(&self, id: Uuid, sequential: bool) -> TorrentResult<()> {
        self.engine.set_sequential(id, sequential).await
    }

    async fn update_limits(&self, id: Option<Uuid>, limits: TorrentRateLimit) -> TorrentResult<()> {
        self.engine.update_limits(id, limits).await
    }

    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> TorrentResult<()> {
        self.engine.update_selection(id, rules).await
    }

    async fn update_options(&self, id: Uuid, options: TorrentOptionsUpdate) -> TorrentResult<()> {
        self.engine.update_options(id, options).await
    }

    async fn update_trackers(
        &self,
        id: Uuid,
        trackers: TorrentTrackersUpdate,
    ) -> TorrentResult<()> {
        self.engine.update_trackers(id, trackers).await
    }

    async fn update_web_seeds(
        &self,
        id: Uuid,
        web_seeds: TorrentWebSeedsUpdate,
    ) -> TorrentResult<()> {
        self.engine.update_web_seeds(id, web_seeds).await
    }

    async fn reannounce(&self, id: Uuid) -> TorrentResult<()> {
        self.engine.reannounce(id).await
    }

    async fn move_torrent(&self, id: Uuid, download_dir: String) -> TorrentResult<()> {
        self.engine.move_torrent(id, download_dir).await
    }

    async fn recheck(&self, id: Uuid) -> TorrentResult<()> {
        self.engine.recheck(id).await
    }
}

#[cfg(any(feature = "libtorrent", test))]
#[async_trait]
impl<E> TorrentInspector for TorrentOrchestrator<E>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    async fn list(&self) -> TorrentResult<Vec<TorrentStatus>> {
        Ok(self.catalog.list().await)
    }

    async fn get(&self, id: Uuid) -> TorrentResult<Option<TorrentStatus>> {
        Ok(self.catalog.get(id).await)
    }

    async fn peers(&self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
        self.engine.peers(id).await
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
            comment: None,
            source: None,
            private: None,
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
                eta_seconds,
                download_bps,
                upload_bps,
                ratio,
            } => {
                let update = ProgressUpdate {
                    bytes_downloaded: *bytes_downloaded,
                    bytes_total: *bytes_total,
                    eta_seconds: *eta_seconds,
                    download_bps: *download_bps,
                    upload_bps: *upload_bps,
                    ratio: *ratio,
                };
                Self::record_progress(entries, *torrent_id, &update);
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
                comment,
                source,
                private,
            } => {
                Self::record_metadata(
                    entries,
                    *torrent_id,
                    name.as_deref(),
                    download_dir.as_deref(),
                    comment.as_deref(),
                    source.as_deref(),
                    *private,
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
        entry.comment = None;
        entry.source = None;
        entry.private = None;
        entry.rates = TorrentRates::default();
        entry.added_at = now;
        entry.last_updated = now;
    }

    fn record_metadata(
        entries: &mut HashMap<Uuid, TorrentStatus>,
        torrent_id: Uuid,
        name: Option<&str>,
        download_dir: Option<&str>,
        comment: Option<&str>,
        source: Option<&str>,
        private: Option<bool>,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        if let Some(name) = name {
            entry.name = Some(name.to_owned());
        }
        if let Some(download_dir) = download_dir {
            entry.download_dir = Some(download_dir.to_owned());
        }
        if let Some(comment) = comment {
            entry.comment = Some(comment.to_owned());
        }
        if let Some(source) = source {
            entry.source = Some(source.to_owned());
        }
        if private.is_some() {
            entry.private = private;
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
        update: &ProgressUpdate,
    ) {
        let entry = Self::ensure_entry(entries, torrent_id);
        entry.progress.bytes_downloaded = update.bytes_downloaded;
        entry.progress.bytes_total = update.bytes_total;
        entry.progress.eta_seconds = update.eta_seconds;
        entry.rates.download_bps = update.download_bps;
        entry.rates.upload_bps = update.upload_bps;
        entry.rates.ratio = if update.ratio.is_finite() {
            update.ratio
        } else {
            0.0
        };
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

fn app_error_to_torrent(
    operation: &'static str,
    torrent_id: Option<Uuid>,
    err: AppError,
) -> TorrentError {
    TorrentError::OperationFailed {
        operation,
        torrent_id,
        source: Box::new(err),
    }
}

fn enforce_allow_paths(root: &Path, allow_paths: &[String]) -> AppResult<()> {
    let allows = parse_path_list(allow_paths)?;
    if allows.is_empty() {
        return Ok(());
    }

    if allows.iter().any(|allow| root.starts_with(allow)) {
        return Ok(());
    }

    let root_abs = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for allow in &allows {
        let allow_abs = allow.canonicalize().unwrap_or_else(|_| allow.clone());
        if root_abs.starts_with(&allow_abs) {
            return Ok(());
        }
    }

    Err(AppError::InvalidConfig {
        field: "allow_paths",
        reason: "root_not_permitted",
        value: Some(root_abs.to_string_lossy().into_owned()),
    })
}

fn parse_path_list(entries: &[String]) -> AppResult<Vec<PathBuf>> {
    entries
        .iter()
        .map(|entry| {
            if entry.trim().is_empty() {
                Err(AppError::InvalidConfig {
                    field: "allow_paths",
                    reason: "empty_entry",
                    value: Some(entry.clone()),
                })
            } else {
                Ok(PathBuf::from(entry))
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "orchestrator/tests.rs"]
mod tests;
