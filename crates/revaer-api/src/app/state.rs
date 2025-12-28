//! API application state, health tracking, and helpers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use revaer_config::ApiKeyRateLimit;
use revaer_events::{Event as CoreEvent, EventBus, TorrentState};
use revaer_telemetry::Metrics;
use revaer_torrent_core::TorrentStatus;
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use crate::TorrentHandles;
use crate::config::ConfigFacade;
use crate::http::rate_limit::{RateLimitError, RateLimitSnapshot, RateLimiter};
use crate::http::torrents::TorrentMetadata;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use revaer_config::{AppMode, AppProfile};
    use revaer_torrent_core::{
        AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
        TorrentWorkflow,
    };
    use serde_json::json;
    use tokio::runtime::Runtime;
    use tokio::sync::RwLock;
    use tokio_stream::StreamExt;

    #[derive(Clone, Default)]
    struct NoopConfig;

    #[async_trait]
    impl ConfigFacade for NoopConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "test".to_string(),
                mode: AppMode::Active,
                version: 1,
                http_port: 8080,
                bind_addr: std::net::IpAddr::from([127, 0, 0, 1]),
                telemetry: json!({}),
                features: json!({}),
                immutable_keys: json!([]),
            })
        }

        async fn issue_setup_token(
            &self,
            _: Duration,
            _: &str,
        ) -> Result<revaer_config::SetupToken> {
            Err(anyhow::anyhow!("not implemented"))
        }

        async fn validate_setup_token(&self, _: &str) -> Result<()> {
            Err(anyhow::anyhow!("not implemented"))
        }

        async fn consume_setup_token(&self, _: &str) -> Result<()> {
            Err(anyhow::anyhow!("not implemented"))
        }

        async fn apply_changeset(
            &self,
            _: &str,
            _: &str,
            _: revaer_config::SettingsChangeset,
        ) -> Result<revaer_config::AppliedChanges> {
            Err(anyhow::anyhow!("not implemented"))
        }

        async fn snapshot(&self) -> Result<revaer_config::ConfigSnapshot> {
            Err(anyhow::anyhow!("not implemented"))
        }

        async fn authenticate_api_key(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<revaer_config::ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> Result<bool> {
            Ok(false)
        }

        async fn factory_reset(&self) -> Result<()> {
            Err(anyhow::anyhow!("not implemented"))
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
        async fn add_torrent(&self, _: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _: Uuid, _: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn pause_torrent(&self, _: Uuid) -> anyhow::Result<()> {
            Ok(())
        }

        async fn resume_torrent(&self, _: Uuid) -> anyhow::Result<()> {
            Ok(())
        }

        async fn set_sequential(&self, _: Uuid, _: bool) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_limits(&self, _: Option<Uuid>, _: TorrentRateLimit) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_selection(&self, _: Uuid, _: FileSelectionUpdate) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_trackers(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::TorrentTrackersUpdate,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn update_web_seeds(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::TorrentWebSeedsUpdate,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn reannounce(&self, _: Uuid) -> anyhow::Result<()> {
            Ok(())
        }

        async fn recheck(&self, _: Uuid) -> anyhow::Result<()> {
            Ok(())
        }

        async fn set_piece_deadline(
            &self,
            _: Uuid,
            _: revaer_torrent_core::model::PieceDeadline,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl revaer_torrent_core::TorrentInspector for RecordingWorkflow {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(self.statuses.read().await.clone())
        }

        async fn get(&self, _: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(None)
        }

        async fn peers(&self, id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
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
    fn add_and_remove_degraded_components_emit_events() {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new().expect("metrics");
        let state = ApiState::new(
            Arc::new(NoopConfig),
            metrics,
            Arc::new(json!({})),
            events.clone(),
            None,
        );
        let runtime = Runtime::new().expect("runtime");
        let mut stream = events.subscribe(None);

        assert!(state.add_degraded_component("db"));
        assert!(!state.add_degraded_component("db"));

        let envelope = runtime
            .block_on(async { stream.next().await })
            .expect("health event emitted")
            .expect("stream recv error");
        assert!(matches!(envelope.event, CoreEvent::HealthChanged { .. }));
        assert!(state.remove_degraded_component("db"));
    }

    #[tokio::test]
    async fn update_torrent_metrics_handles_stub_handles() {
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
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            Some(handles),
        );

        state.update_torrent_metrics().await;
    }

    #[test]
    fn update_metadata_inserts_defaults() {
        let id = Uuid::new_v4();
        let state = ApiState::new(
            Arc::new(NoopConfig),
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        );

        state.update_metadata(&id, |metadata| metadata.tags.push("tag-a".into()));
        let metadata = state.get_metadata(&id);
        assert_eq!(metadata.tags, vec!["tag-a".to_string()]);
        assert!(metadata.selection.priorities.is_empty());
    }
}

pub(crate) struct ApiState {
    pub(crate) config: Arc<dyn ConfigFacade>,
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

impl ApiState {
    pub(crate) fn new(
        config: Arc<dyn ConfigFacade>,
        telemetry: Metrics,
        openapi_document: Arc<Value>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
    ) -> Self {
        Self {
            config,
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
        let _ = self
            .events
            .publish(CoreEvent::HealthChanged { degraded: snapshot });
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
        let _ = self
            .events
            .publish(CoreEvent::HealthChanged { degraded: snapshot });
        true
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
        mutex.lock().unwrap_or_else(|err| {
            panic!("failed to lock {name}: {err}");
        })
    }
}
