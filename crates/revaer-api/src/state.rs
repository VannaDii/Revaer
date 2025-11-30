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
use crate::http::torrents::TorrentMetadata;
use crate::rate_limit::{RateLimitError, RateLimitSnapshot, RateLimiter};

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
    compat_sessions: Mutex<HashMap<String, CompatSession>>,
}

#[derive(Clone)]
pub(crate) struct CompatSession {
    pub(crate) expires_at: Instant,
}

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
