//! API application state, health tracking, and helpers.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use revaer_config::ApiKeyRateLimit;
use revaer_events::{Event as CoreEvent, EventBus, TorrentState};
use revaer_telemetry::Metrics;
use revaer_torrent_core::TorrentStatus;
use serde_json::Value;
use systemstat::{Platform, System};
use tracing::{error, warn};
use uuid::Uuid;

use crate::TorrentHandles;
use crate::app::indexers::IndexerFacade;
use crate::config::ConfigFacade;
use crate::http::rate_limit::{RateLimitError, RateLimitSnapshot, RateLimiter};
use crate::http::torrents::TorrentMetadata;
use crate::models::DashboardResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::test_indexers;
    use anyhow::Result;
    use async_trait::async_trait;
    use revaer_config::{
        AppMode, AppProfile, ConfigError, ConfigResult, TelemetryConfig,
        validate::default_local_networks,
    };
    use revaer_torrent_core::{
        AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
        TorrentResult, TorrentWorkflow,
    };
    use serde_json::json;
    use tokio::runtime::Runtime;
    use tokio::sync::RwLock;
    use tokio_stream::StreamExt;

    #[derive(Clone, Default)]
    struct NoopConfig;

    #[async_trait]
    impl ConfigFacade for NoopConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "test".to_string(),
                mode: AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 8080,
                bind_addr: std::net::IpAddr::from([127, 0, 0, 1]),
                local_networks: default_local_networks(),
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            })
        }

        async fn issue_setup_token(
            &self,
            _: Duration,
            _: &str,
        ) -> ConfigResult<revaer_config::SetupToken> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn validate_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn consume_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "setup_token".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn apply_changeset(
            &self,
            _: &str,
            _: &str,
            _: revaer_config::SettingsChangeset,
        ) -> ConfigResult<revaer_config::AppliedChanges> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "changeset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn snapshot(&self) -> ConfigResult<revaer_config::ConfigSnapshot> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "snapshot".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn authenticate_api_key(
            &self,
            _: &str,
            _: &str,
        ) -> ConfigResult<Option<revaer_config::ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(false)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "factory_reset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }
    }

    #[derive(Default)]
    struct RecordingWorkflow {
        statuses: RwLock<Vec<TorrentStatus>>,
        peers: RwLock<HashMap<Uuid, Vec<PeerSnapshot>>>,
    }

    impl RecordingWorkflow {
        fn with_status(status: TorrentStatus) -> Arc<Self> {
            Self {
                statuses: RwLock::new(vec![status]),
                peers: RwLock::new(HashMap::new()),
            }
            .into()
        }
    }

    #[async_trait]
    impl TorrentWorkflow for RecordingWorkflow {
        async fn add_torrent(&self, _: AddTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _: Uuid, _: RemoveTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn pause_torrent(&self, _: Uuid) -> TorrentResult<()> {
            Ok(())
        }

        async fn resume_torrent(&self, _: Uuid) -> TorrentResult<()> {
            Ok(())
        }

        async fn set_sequential(&self, _: Uuid, _: bool) -> TorrentResult<()> {
            Ok(())
        }

        async fn update_limits(&self, _: Option<Uuid>, _: TorrentRateLimit) -> TorrentResult<()> {
            Ok(())
        }

        async fn update_selection(&self, _: Uuid, _: FileSelectionUpdate) -> TorrentResult<()> {
            Ok(())
        }

        async fn update_trackers(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::TorrentTrackersUpdate,
        ) -> TorrentResult<()> {
            Ok(())
        }

        async fn update_web_seeds(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::TorrentWebSeedsUpdate,
        ) -> TorrentResult<()> {
            Ok(())
        }

        async fn reannounce(&self, _: Uuid) -> TorrentResult<()> {
            Ok(())
        }

        async fn recheck(&self, _: Uuid) -> TorrentResult<()> {
            Ok(())
        }

        async fn set_piece_deadline(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::PieceDeadline,
        ) -> TorrentResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl revaer_torrent_core::TorrentInspector for RecordingWorkflow {
        async fn list(&self) -> TorrentResult<Vec<TorrentStatus>> {
            Ok(self.statuses.read().await.clone())
        }

        async fn get(&self, _: Uuid) -> TorrentResult<Option<TorrentStatus>> {
            Ok(None)
        }

        async fn peers(&self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
            Ok(self
                .peers
                .read()
                .await
                .get(&id)
                .cloned()
                .unwrap_or_default())
        }
    }

    #[test]
    fn add_and_remove_degraded_components_emit_events() -> Result<()> {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new()?;
        let state = ApiState::new(
            Arc::new(NoopConfig),
            test_indexers(),
            metrics,
            Arc::new(json!({})),
            events.clone(),
            None,
        );
        let runtime = Runtime::new()?;
        let mut stream = events.subscribe(None);

        assert!(state.add_degraded_component("db"));
        assert!(!state.add_degraded_component("db"));

        let envelope = runtime
            .block_on(async { stream.next().await })
            .ok_or_else(|| anyhow::anyhow!("health event missing"))??;
        assert!(matches!(envelope.event, CoreEvent::HealthChanged { .. }));
        assert!(state.remove_degraded_component("db"));
        Ok(())
    }

    #[tokio::test]
    async fn update_torrent_metrics_handles_stub_handles() -> Result<()> {
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("demo".into()),
            state: TorrentState::Completed,
            progress: revaer_torrent_core::TorrentProgress::default(),
            rates: revaer_torrent_core::TorrentRates::default(),
            files: None,
            library_path: None,
            download_dir: None,
            comment: None,
            source: None,
            private: None,
            sequential: false,
            added_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            last_updated: chrono::Utc::now(),
        };
        let workflow = RecordingWorkflow::with_status(status);
        let handles = TorrentHandles::new(workflow.clone(), workflow);
        let state = ApiState::new(
            Arc::new(NoopConfig),
            test_indexers(),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            Some(handles),
        );

        state.update_torrent_metrics().await;
        Ok(())
    }

    #[test]
    fn update_metadata_inserts_defaults() -> Result<()> {
        let id = Uuid::new_v4();
        let state = ApiState::new(
            Arc::new(NoopConfig),
            test_indexers(),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );

        state.update_metadata(&id, |metadata| metadata.tags.push("tag-a".into()));
        let metadata = state.get_metadata(&id);
        assert_eq!(metadata.tags, vec!["tag-a".to_string()]);
        assert!(metadata.selection.priorities.is_empty());
        Ok(())
    }

    #[test]
    fn metadata_can_be_set_and_removed() -> Result<()> {
        let id = Uuid::new_v4();
        let state = ApiState::new(
            Arc::new(NoopConfig),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );

        let mut metadata = TorrentMetadata::default();
        metadata.tags.push("alpha".to_string());
        state.set_metadata(id, metadata);
        assert_eq!(state.get_metadata(&id).tags, vec!["alpha".to_string()]);

        state.remove_metadata(&id);
        assert!(state.get_metadata(&id).tags.is_empty());
        Ok(())
    }

    #[test]
    fn rate_limit_guard_tracks_missing_limit_and_active_limit() -> Result<()> {
        let state = ApiState::new(
            Arc::new(NoopConfig),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );

        let missing = state.enforce_rate_limit("demo", None)?;
        assert!(missing.is_none());
        assert_eq!(
            state.current_health_degraded(),
            vec!["api_rate_limit_guard".to_string()]
        );

        let limit = ApiKeyRateLimit {
            burst: 2,
            replenish_period: Duration::from_secs(60),
        };
        let snapshot = state
            .enforce_rate_limit("demo", Some(&limit))?
            .ok_or_else(|| anyhow::anyhow!("expected snapshot"))?;
        assert_eq!(snapshot.limit, 2);
        assert!(snapshot.remaining <= 1);
        assert!(state.current_health_degraded().is_empty());
        Ok(())
    }

    #[test]
    fn rate_limit_enforcement_rejects_when_burst_exhausted() -> Result<()> {
        let state = ApiState::new(
            Arc::new(NoopConfig),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );
        let limit = ApiKeyRateLimit {
            burst: 1,
            replenish_period: Duration::from_secs(60),
        };
        assert!(state.enforce_rate_limit("demo", Some(&limit))?.is_some());
        let Err(err) = state.enforce_rate_limit("demo", Some(&limit)) else {
            return Err(anyhow::anyhow!("second request should be rate limited"));
        };
        assert_eq!(err.limit, 1);
        assert!(err.retry_after.as_secs() <= 60);
        Ok(())
    }
}

pub(crate) struct ApiState {
    pub(crate) config: Arc<dyn ConfigFacade>,
    pub(crate) indexers: Arc<dyn IndexerFacade>,
    pub(crate) setup_token_ttl: Duration,
    pub(crate) telemetry: Metrics,
    pub(crate) openapi_document: Arc<Value>,
    pub(crate) events: EventBus,
    health_status: Mutex<Vec<String>>,
    rate_limiters: Mutex<HashMap<String, RateLimiter>>,
    torrent_metadata: Mutex<HashMap<Uuid, TorrentMetadata>>,
    pub(crate) torrent: Option<TorrentHandles>,
    #[cfg(feature = "compat-qb")]
    compat_sessions: Mutex<HashMap<String, CompatSession>>,
}

#[cfg(feature = "compat-qb")]
#[derive(Clone)]
pub(crate) struct CompatSession {
    pub(crate) expires_at: Instant,
}

#[cfg(feature = "compat-qb")]
pub(crate) const COMPAT_SESSION_TTL: Duration = Duration::from_secs(30 * 60);
const DASHBOARD_TORRENTS_COMPONENT: &str = "dashboard_torrents";
const DASHBOARD_DISK_COMPONENT: &str = "dashboard_disk";

impl ApiState {
    pub(crate) fn new(
        config: Arc<dyn ConfigFacade>,
        indexers: Arc<dyn IndexerFacade>,
        telemetry: Metrics,
        openapi_document: Arc<Value>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
    ) -> Self {
        Self {
            config,
            indexers,
            setup_token_ttl: Duration::from_secs(900),
            telemetry,
            openapi_document,
            events,
            health_status: Mutex::new(Vec::new()),
            rate_limiters: Mutex::new(HashMap::new()),
            torrent_metadata: Mutex::new(HashMap::new()),
            torrent,
            #[cfg(feature = "compat-qb")]
            compat_sessions: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn add_degraded_component(&self, component: &str) -> bool {
        let mut guard = Self::lock_guard(&self.health_status, "health_status");
        if guard.iter().any(|entry| entry == component) {
            return false;
        }
        guard.push(component.to_string());
        guard.sort();
        guard.dedup();
        let snapshot = guard.clone();
        drop(guard);
        self.publish_event(CoreEvent::HealthChanged { degraded: snapshot });
        true
    }

    pub(crate) fn remove_degraded_component(&self, component: &str) -> bool {
        let mut guard = Self::lock_guard(&self.health_status, "health_status");
        let previous = guard.len();
        guard.retain(|entry| entry != component);
        if guard.len() == previous {
            return false;
        }
        let snapshot = guard.clone();
        drop(guard);
        self.publish_event(CoreEvent::HealthChanged { degraded: snapshot });
        true
    }

    pub(crate) fn publish_event(&self, event: CoreEvent) {
        if let Err(error) = self.events.publish(event) {
            warn!(
                event_id = error.event_id(),
                event_kind = error.event_kind(),
                error = %error,
                "failed to publish event"
            );
        }
    }

    pub(crate) fn record_torrent_metrics(&self, statuses: &[TorrentStatus]) {
        let active = i64::try_from(statuses.len()).unwrap_or(i64::MAX);
        let queued = i64::try_from(
            statuses
                .iter()
                .filter(|status| matches!(status.state, TorrentState::Queued))
                .count(),
        )
        .unwrap_or(i64::MAX);
        self.telemetry.set_active_torrents(active);
        self.telemetry.set_queue_depth(queued);
    }

    pub(crate) async fn update_torrent_metrics(&self) {
        if let Some(handles) = &self.torrent {
            match handles.inspector().list().await {
                Ok(statuses) => {
                    self.record_torrent_metrics(&statuses);
                }
                Err(err) => {
                    warn!(error = %err, "failed to refresh torrent metrics");
                }
            }
        } else {
            self.record_torrent_metrics(&[]);
        }
    }

    pub(crate) async fn dashboard_snapshot(&self, library_root: &Path) -> DashboardResponse {
        let statuses = self.dashboard_statuses().await;
        let (download_bps, upload_bps, active, paused, completed) =
            aggregate_dashboard_counts(&statuses);
        let (disk_total_gb, disk_used_gb) = match dashboard_disk_usage_gb(library_root) {
            Ok(snapshot) => {
                self.remove_degraded_component(DASHBOARD_DISK_COMPONENT);
                snapshot
            }
            Err(err) => {
                self.add_degraded_component(DASHBOARD_DISK_COMPONENT);
                warn!(
                    error = %err,
                    library_root = %library_root.display(),
                    "failed to read dashboard disk usage"
                );
                (0, 0)
            }
        };

        DashboardResponse {
            download_bps,
            upload_bps,
            active,
            paused,
            completed,
            disk_total_gb,
            disk_used_gb,
        }
    }

    async fn dashboard_statuses(&self) -> Vec<TorrentStatus> {
        if let Some(handles) = &self.torrent {
            match handles.inspector().list().await {
                Ok(statuses) => {
                    self.remove_degraded_component(DASHBOARD_TORRENTS_COMPONENT);
                    statuses
                }
                Err(err) => {
                    self.add_degraded_component(DASHBOARD_TORRENTS_COMPONENT);
                    warn!(error = %err, "failed to list torrents for dashboard snapshot");
                    Vec::new()
                }
            }
        } else {
            self.remove_degraded_component(DASHBOARD_TORRENTS_COMPONENT);
            Vec::new()
        }
    }

    pub(crate) fn current_health_degraded(&self) -> Vec<String> {
        Self::lock_guard(&self.health_status, "health_status").clone()
    }

    pub(crate) fn enforce_rate_limit(
        &self,
        key_id: &str,
        limit: Option<&ApiKeyRateLimit>,
    ) -> Result<Option<RateLimitSnapshot>, RateLimitError> {
        limit.map_or_else(
            || {
                if self.add_degraded_component("api_rate_limit_guard") {
                    self.telemetry.inc_guardrail_violation();
                    warn!("api key guard rail triggered: missing or unlimited rate limit");
                }
                Ok(None)
            },
            |limit| {
                self.remove_degraded_component("api_rate_limit_guard");
                let mut guard = Self::lock_guard(&self.rate_limiters, "rate_limiters");
                let limiter = guard
                    .entry(key_id.to_string())
                    .or_insert_with(|| RateLimiter::new(limit.clone()));
                let now = Instant::now();
                let status = limiter.evaluate(limit, now);
                drop(guard);
                if status.allowed {
                    Ok(Some(RateLimitSnapshot {
                        limit: limit.burst,
                        remaining: status.remaining,
                    }))
                } else {
                    self.telemetry.inc_rate_limit_throttled();
                    warn!(api_key = %key_id, "API key rate limit exceeded");
                    Err(RateLimitError {
                        limit: limit.burst,
                        retry_after: status.retry_after,
                    })
                }
            },
        )
    }

    pub(crate) fn set_metadata(&self, id: Uuid, metadata: TorrentMetadata) {
        let mut guard = Self::lock_guard(&self.torrent_metadata, "torrent_metadata");
        guard.insert(id, metadata);
    }

    pub(crate) fn update_metadata(&self, id: &Uuid, update: impl FnOnce(&mut TorrentMetadata)) {
        update(
            Self::lock_guard(&self.torrent_metadata, "torrent_metadata")
                .entry(*id)
                .or_default(),
        );
    }

    pub(crate) fn get_metadata(&self, id: &Uuid) -> TorrentMetadata {
        Self::lock_guard(&self.torrent_metadata, "torrent_metadata")
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn remove_metadata(&self, id: &Uuid) {
        let mut guard = Self::lock_guard(&self.torrent_metadata, "torrent_metadata");
        guard.remove(id);
    }

    #[cfg(feature = "compat-qb")]
    pub(crate) fn issue_qb_session(&self) -> String {
        let session_id = uuid::Uuid::new_v4().simple().to_string();
        let mut guard = Self::lock_guard(&self.compat_sessions, "compat_sessions");
        guard.insert(
            session_id.clone(),
            CompatSession {
                expires_at: Instant::now() + COMPAT_SESSION_TTL,
            },
        );
        session_id
    }

    #[cfg(feature = "compat-qb")]
    pub(crate) fn validate_qb_session(&self, session_id: &str) -> bool {
        let mut guard = Self::lock_guard(&self.compat_sessions, "compat_sessions");
        if let Some(session) = guard.get(session_id)
            && session.expires_at > Instant::now()
        {
            return true;
        }
        guard.remove(session_id);
        false
    }

    #[cfg(feature = "compat-qb")]
    pub(crate) fn revoke_qb_session(&self, session_id: &str) {
        let mut guard = Self::lock_guard(&self.compat_sessions, "compat_sessions");
        guard.remove(session_id);
    }

    fn lock_guard<'a, T>(mutex: &'a Mutex<T>, name: &'a str) -> MutexGuard<'a, T> {
        match mutex.lock() {
            Ok(guard) => guard,
            Err(err) => {
                error!(mutex = name, error = ?err, "mutex poisoned");
                err.into_inner()
            }
        }
    }
}

fn aggregate_dashboard_counts(statuses: &[TorrentStatus]) -> (u64, u64, u32, u32, u32) {
    let mut download_bps = 0_u64;
    let mut upload_bps = 0_u64;
    let mut active = 0_u32;
    let mut paused = 0_u32;
    let mut completed = 0_u32;

    for status in statuses {
        download_bps = download_bps.saturating_add(status.rates.download_bps);
        upload_bps = upload_bps.saturating_add(status.rates.upload_bps);
        match status.state {
            TorrentState::FetchingMetadata | TorrentState::Downloading | TorrentState::Seeding => {
                active = active.saturating_add(1);
            }
            TorrentState::Queued | TorrentState::Stopped => {
                paused = paused.saturating_add(1);
            }
            TorrentState::Completed => {
                completed = completed.saturating_add(1);
            }
            TorrentState::Failed { .. } => {}
        }
    }

    (download_bps, upload_bps, active, paused, completed)
}

fn dashboard_disk_usage_gb(path: &Path) -> std::io::Result<(u32, u32)> {
    let mount = System::new().mount_at(path)?;
    let total_bytes = mount.total.as_u64();
    let used_bytes = total_bytes.saturating_sub(mount.avail.as_u64());
    Ok((
        bytes_to_whole_gb(total_bytes),
        bytes_to_whole_gb(used_bytes),
    ))
}

fn bytes_to_whole_gb(bytes: u64) -> u32 {
    let gigabytes = bytes / 1_000_000_000;
    u32::try_from(gigabytes).unwrap_or(u32::MAX)
}
