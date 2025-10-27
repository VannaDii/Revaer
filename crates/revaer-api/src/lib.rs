#![allow(unexpected_cfgs)]

pub mod models;

#[cfg(feature = "compat-qb")]
pub(crate) mod compat_qb;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use std::collections::{HashMap, HashSet};
use std::convert::{Infallible, TryFrom};
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, Instant};

use anyhow::Result;
use async_stream::stream;
use axum::{
    Json, Router,
    body::Body,
    extract::{Extension, MatchedPath, Path as AxumPath, Query, State},
    http::{HeaderMap, Request, StatusCode, header::CONTENT_TYPE},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{self, Sse},
    },
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, future};
use models::{
    ProblemDetails, ProblemInvalidParam, TorrentAction, TorrentCreateRequest, TorrentDetail,
    TorrentListResponse, TorrentSelectionRequest, TorrentStateKind, TorrentSummary,
};
use revaer_config::{
    ApiKeyAuth, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigService,
    ConfigSnapshot, SettingsChangeset, SetupToken,
};
use revaer_events::{Event as CoreEvent, EventBus, EventEnvelope, EventId, TorrentState};
use revaer_telemetry::{
    Metrics, build_sha, record_app_mode, set_request_context, with_request_context,
};
use revaer_torrent_core::{
    AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentInspector, TorrentRateLimit,
    TorrentSource, TorrentStatus, TorrentWorkflow,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::net::TcpListener;
use tower::{Service, ServiceBuilder, layer::Layer};
use tower_http::trace::TraceLayer;
use tracing::{Span, error, info, warn};
use uuid::Uuid;

const HEADER_SETUP_TOKEN: &str = "x-revaer-setup-token";
const HEADER_API_KEY: &str = "x-revaer-api-key";
const HEADER_REQUEST_ID: &str = "x-request-id";
const HEADER_LAST_EVENT_ID: &str = "last-event-id";
const SSE_KEEP_ALIVE_SECS: u64 = 20;

const PROBLEM_INTERNAL: &str = "https://revaer.dev/problems/internal";
const PROBLEM_UNAUTHORIZED: &str = "https://revaer.dev/problems/unauthorized";
const PROBLEM_BAD_REQUEST: &str = "https://revaer.dev/problems/bad-request";
const PROBLEM_CONFLICT: &str = "https://revaer.dev/problems/conflict";
const PROBLEM_CONFIG_INVALID: &str = "https://revaer.dev/problems/config-invalid";
const PROBLEM_SETUP_REQUIRED: &str = "https://revaer.dev/problems/setup-required";
const PROBLEM_SERVICE_UNAVAILABLE: &str = "https://revaer.dev/problems/service-unavailable";
const PROBLEM_NOT_FOUND: &str = "https://revaer.dev/problems/not-found";
const PROBLEM_RATE_LIMITED: &str = "https://revaer.dev/problems/rate-limited";
const MAX_METAINFO_BYTES: usize = 5 * 1024 * 1024;
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const EVENT_KIND_WHITELIST: &[&str] = &[
    "torrent_added",
    "files_discovered",
    "progress",
    "state_changed",
    "completed",
    "fsops_started",
    "fsops_progress",
    "fsops_completed",
    "fsops_failed",
    "settings_changed",
    "health_changed",
    "selection_reconciled",
];

#[derive(Clone)]
pub struct TorrentHandles {
    workflow: Arc<dyn TorrentWorkflow>,
    inspector: Arc<dyn TorrentInspector>,
}

impl TorrentHandles {
    pub fn new(workflow: Arc<dyn TorrentWorkflow>, inspector: Arc<dyn TorrentInspector>) -> Self {
        Self {
            workflow,
            inspector,
        }
    }

    #[must_use]
    pub fn workflow(&self) -> &Arc<dyn TorrentWorkflow> {
        &self.workflow
    }

    #[must_use]
    pub fn inspector(&self) -> &Arc<dyn TorrentInspector> {
        &self.inspector
    }
}

pub struct ApiServer {
    router: Router,
}

#[async_trait]
pub trait ConfigFacade: Send + Sync {
    async fn get_app_profile(&self) -> Result<AppProfile>;
    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken>;
    async fn validate_setup_token(&self, token: &str) -> Result<()>;
    async fn consume_setup_token(&self, token: &str) -> Result<()>;
    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges>;
    async fn snapshot(&self) -> Result<ConfigSnapshot>;
    async fn authenticate_api_key(&self, key_id: &str, secret: &str) -> Result<Option<ApiKeyAuth>>;
}

type SharedConfig = Arc<dyn ConfigFacade>;

#[async_trait]
impl ConfigFacade for ConfigService {
    async fn get_app_profile(&self) -> Result<AppProfile> {
        Self::get_app_profile(self).await
    }

    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken> {
        Self::issue_setup_token(self, ttl, issued_by).await
    }

    async fn validate_setup_token(&self, token: &str) -> Result<()> {
        Self::validate_setup_token(self, token).await
    }

    async fn consume_setup_token(&self, token: &str) -> Result<()> {
        Self::consume_setup_token(self, token).await
    }

    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges> {
        Self::apply_changeset(self, actor, reason, changeset).await
    }

    async fn snapshot(&self) -> Result<ConfigSnapshot> {
        Self::snapshot(self).await
    }

    async fn authenticate_api_key(&self, key_id: &str, secret: &str) -> Result<Option<ApiKeyAuth>> {
        Self::authenticate_api_key(self, key_id, secret).await
    }
}

struct ApiState {
    config: SharedConfig,
    setup_token_ttl: Duration,
    telemetry: Metrics,
    openapi_document: Arc<Value>,
    events: EventBus,
    health_status: Mutex<Vec<String>>,
    rate_limiters: Mutex<HashMap<String, RateLimiter>>,
    torrent_metadata: Mutex<HashMap<Uuid, TorrentMetadata>>,
    torrent: Option<TorrentHandles>,
}

impl ApiState {
    fn new(
        config: SharedConfig,
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
        }
    }

    fn add_degraded_component(&self, component: &str) -> bool {
        let mut guard = self
            .health_status
            .lock()
            .expect("health status mutex poisoned");
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

    fn remove_degraded_component(&self, component: &str) -> bool {
        let mut guard = self
            .health_status
            .lock()
            .expect("health status mutex poisoned");
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

    fn record_torrent_metrics(&self, statuses: &[TorrentStatus]) {
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

    async fn update_torrent_metrics(&self) {
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

    fn current_health_degraded(&self) -> Vec<String> {
        self.health_status
            .lock()
            .expect("health status mutex poisoned")
            .clone()
    }

    fn enforce_rate_limit(
        &self,
        key_id: &str,
        limit: Option<&ApiKeyRateLimit>,
    ) -> Result<(), ApiError> {
        limit.map_or_else(
            || {
                if self.add_degraded_component("api_rate_limit_guard") {
                    self.telemetry.inc_guardrail_violation();
                    warn!("api key guard rail triggered: missing or unlimited rate limit");
                }
                Ok(())
            },
            |limit| {
                self.remove_degraded_component("api_rate_limit_guard");
                let mut guard = self
                    .rate_limiters
                    .lock()
                    .expect("rate limiters mutex poisoned");
                let limiter = guard
                    .entry(key_id.to_string())
                    .or_insert_with(|| RateLimiter::new(limit.clone()));
                let now = Instant::now();
                let allowed = limiter.allow(limit, now);
                drop(guard);
                if allowed {
                    Ok(())
                } else {
                    self.telemetry.inc_rate_limit_throttled();
                    warn!(api_key = %key_id, "API key rate limit exceeded");
                    Err(ApiError::too_many_requests(
                        "API key rate limit exceeded; try again later",
                    ))
                }
            },
        )
    }

    fn set_metadata(&self, id: Uuid, metadata: TorrentMetadata) {
        let mut guard = self
            .torrent_metadata
            .lock()
            .expect("torrent metadata mutex poisoned");
        guard.insert(id, metadata);
    }

    fn get_metadata(&self, id: &Uuid) -> TorrentMetadata {
        self.torrent_metadata
            .lock()
            .expect("torrent metadata mutex poisoned")
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    fn remove_metadata(&self, id: &Uuid) {
        self.torrent_metadata
            .lock()
            .expect("torrent metadata mutex poisoned")
            .remove(id);
    }
}

#[derive(Clone, Default)]
struct TorrentMetadata {
    tags: Vec<String>,
    trackers: Vec<String>,
}

impl TorrentMetadata {
    #[allow(clippy::missing_const_for_fn)]
    fn new(tags: Vec<String>, trackers: Vec<String>) -> Self {
        Self { tags, trackers }
    }
}

fn summary_from_components(status: TorrentStatus, metadata: TorrentMetadata) -> TorrentSummary {
    TorrentSummary::from(status).with_metadata(metadata.tags, metadata.trackers)
}

fn detail_from_components(status: TorrentStatus, metadata: TorrentMetadata) -> TorrentDetail {
    let mut detail = TorrentDetail::from(status);
    detail.summary = detail
        .summary
        .with_metadata(metadata.tags, metadata.trackers);
    detail
}

#[derive(Debug, Default, Deserialize)]
struct TorrentListQuery {
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    tracker: Option<String>,
    #[serde(default)]
    extension: Option<String>,
    #[serde(default)]
    tags: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Clone)]
struct StatusEntry {
    status: TorrentStatus,
    metadata: TorrentMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct CursorToken {
    last_updated: DateTime<Utc>,
    id: Uuid,
}

#[derive(Debug, Default, Deserialize)]
struct SseQuery {
    #[serde(default)]
    torrent: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct SseFilter {
    torrent_ids: HashSet<Uuid>,
    event_kinds: HashSet<String>,
    states: HashSet<TorrentStateKind>,
}

fn encode_cursor_from_entry(entry: &StatusEntry) -> Result<String, ApiError> {
    let token = CursorToken {
        last_updated: entry.status.last_updated,
        id: entry.status.id,
    };
    let json = serde_json::to_vec(&token).map_err(|err| {
        error!(error = %err, "failed to serialise cursor token");
        ApiError::internal("failed to encode pagination cursor")
    })?;
    Ok(general_purpose::STANDARD.encode(json))
}

fn decode_cursor_token(value: &str) -> Result<CursorToken, ApiError> {
    let bytes = general_purpose::STANDARD
        .decode(value)
        .map_err(|_| ApiError::bad_request("cursor token was not valid base64"))?;
    serde_json::from_slice(&bytes).map_err(|_| ApiError::bad_request("cursor token malformed"))
}

fn parse_state_filter(value: &str) -> Result<TorrentStateKind, ApiError> {
    match value {
        "queued" => Ok(TorrentStateKind::Queued),
        "fetching_metadata" => Ok(TorrentStateKind::FetchingMetadata),
        "downloading" => Ok(TorrentStateKind::Downloading),
        "seeding" => Ok(TorrentStateKind::Seeding),
        "completed" => Ok(TorrentStateKind::Completed),
        "failed" => Ok(TorrentStateKind::Failed),
        "stopped" => Ok(TorrentStateKind::Stopped),
        other => Err(ApiError::bad_request(format!(
            "state filter '{other}' is not recognised"
        ))),
    }
}

fn split_comma_separated(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

fn normalise_lower(value: &str) -> String {
    value.trim().to_lowercase()
}

fn build_sse_filter(query: &SseQuery) -> Result<SseFilter, ApiError> {
    let mut filter = SseFilter::default();

    if let Some(torrent) = query.torrent.as_deref() {
        for value in split_comma_separated(torrent) {
            let parsed = Uuid::parse_str(&value).map_err(|_| {
                ApiError::bad_request(format!("torrent filter '{value}' is not a valid UUID"))
            })?;
            filter.torrent_ids.insert(parsed);
        }
    }

    if let Some(events) = query.event.as_deref() {
        for value in split_comma_separated(events) {
            if !EVENT_KIND_WHITELIST.contains(&value.as_str()) {
                return Err(ApiError::bad_request(format!(
                    "event filter '{value}' is not recognised"
                )));
            }
            filter.event_kinds.insert(value);
        }
    }

    if let Some(states) = query.state.as_deref() {
        for value in split_comma_separated(states) {
            let parsed = parse_state_filter(&value)?;
            filter.states.insert(parsed);
        }
    }

    Ok(filter)
}

fn matches_sse_filter(envelope: &EventEnvelope, filter: &SseFilter) -> bool {
    if !filter.event_kinds.is_empty() && !filter.event_kinds.contains(envelope.event.kind()) {
        return false;
    }

    if !filter.torrent_ids.is_empty() {
        let torrent_id = match &envelope.event {
            CoreEvent::TorrentAdded { torrent_id, .. }
            | CoreEvent::FilesDiscovered { torrent_id, .. }
            | CoreEvent::Progress { torrent_id, .. }
            | CoreEvent::StateChanged { torrent_id, .. }
            | CoreEvent::Completed { torrent_id, .. }
            | CoreEvent::FsopsStarted { torrent_id, .. }
            | CoreEvent::FsopsProgress { torrent_id, .. }
            | CoreEvent::FsopsCompleted { torrent_id, .. }
            | CoreEvent::FsopsFailed { torrent_id, .. }
            | CoreEvent::SelectionReconciled { torrent_id, .. } => torrent_id,
            CoreEvent::SettingsChanged { .. } | CoreEvent::HealthChanged { .. } => {
                return false;
            }
        };

        if !filter.torrent_ids.contains(torrent_id) {
            return false;
        }
    }

    if !filter.states.is_empty() {
        match &envelope.event {
            CoreEvent::StateChanged { state, .. } => {
                let mapped = TorrentStateKind::from(state.clone());
                if !filter.states.contains(&mapped) {
                    return false;
                }
            }
            CoreEvent::Completed { .. } => {
                if !filter.states.contains(&TorrentStateKind::Completed) {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}

#[derive(Clone)]
enum AuthContext {
    SetupToken(String),
    ApiKey { key_id: String },
}

struct RateLimiter {
    config: ApiKeyRateLimit,
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(config: ApiKeyRateLimit) -> Self {
        let capacity = f64::from(config.burst);
        Self {
            config,
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    fn allow(&mut self, config: &ApiKeyRateLimit, now: Instant) -> bool {
        if self.config != *config {
            self.config = config.clone();
            self.tokens = f64::from(config.burst);
            self.last_refill = now;
        }

        let elapsed = now.saturating_duration_since(self.last_refill);
        if elapsed >= self.config.replenish_period {
            self.tokens = f64::from(self.config.burst);
            self.last_refill = now;
        } else if elapsed > Duration::ZERO {
            let refill_rate =
                f64::from(self.config.burst) / self.config.replenish_period.as_secs_f64();
            let replenished = refill_rate * elapsed.as_secs_f64();
            if replenished > 0.0 {
                self.tokens = (self.tokens + replenished).clamp(0.0, f64::from(self.config.burst));
                self.last_refill = now;
            }
        }

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
struct HttpMetricsLayer {
    telemetry: Metrics,
}

impl HttpMetricsLayer {
    const fn new(telemetry: Metrics) -> Self {
        Self { telemetry }
    }
}

impl<S> Layer<S> for HttpMetricsLayer {
    type Service = HttpMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpMetricsService {
            inner,
            telemetry: self.telemetry.clone(),
        }
    }
}

#[derive(Clone)]
struct HttpMetricsService<S> {
    inner: S,
    telemetry: Metrics,
}

impl<S, B> Service<Request<B>> for HttpMetricsService<S>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let route = req.extensions().get::<MatchedPath>().map_or_else(
            || req.uri().path().to_string(),
            |matched| matched.as_str().to_string(),
        );
        let request_id = req
            .headers()
            .get(HEADER_REQUEST_ID)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let telemetry = self.telemetry.clone();
        let fut = self.inner.call(req);

        Box::pin(async move {
            with_request_context(request_id, route.clone(), async move {
                let response = fut.await?;
                telemetry.inc_http_request(&route, response.status().as_u16());
                Ok(response)
            })
            .await
        })
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    kind: &'static str,
    title: &'static str,
    detail: Option<String>,
    invalid_params: Option<Vec<ProblemInvalidParam>>,
}

impl ApiError {
    const fn new(status: StatusCode, kind: &'static str, title: &'static str) -> Self {
        Self {
            status,
            kind,
            title,
            detail: None,
            invalid_params: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn with_invalid_params(mut self, params: Vec<ProblemInvalidParam>) -> Self {
        self.invalid_params = Some(params);
        self
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PROBLEM_INTERNAL,
            "internal server error",
        )
        .with_detail(message)
    }

    fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            PROBLEM_UNAUTHORIZED,
            "authentication required",
        )
        .with_detail(detail)
    }

    fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, PROBLEM_BAD_REQUEST, "bad request").with_detail(detail)
    }

    fn not_found(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            PROBLEM_NOT_FOUND,
            "resource not found",
        )
        .with_detail(detail)
    }

    fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, PROBLEM_CONFLICT, "conflict").with_detail(detail)
    }

    fn setup_required(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            PROBLEM_SETUP_REQUIRED,
            "setup required",
        )
        .with_detail(detail)
    }

    fn config_invalid(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            PROBLEM_CONFIG_INVALID,
            "configuration invalid",
        )
        .with_detail(detail)
    }

    fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            PROBLEM_SERVICE_UNAVAILABLE,
            "service unavailable",
        )
        .with_detail(detail)
    }

    fn too_many_requests(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            PROBLEM_RATE_LIMITED,
            "rate limit exceeded",
        )
        .with_detail(detail)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ProblemDetails {
            kind: self.kind.to_string(),
            title: self.title.to_string(),
            status: self.status.as_u16(),
            detail: self.detail,
            invalid_params: self.invalid_params,
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Debug, Default, Deserialize)]
struct SetupStartRequest {
    issued_by: Option<String>,
    ttl_seconds: Option<u64>,
}

#[derive(Serialize)]
struct SetupStartResponse {
    token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct HealthComponent {
    status: &'static str,
    revision: Option<i64>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    mode: AppMode,
    database: HealthComponent,
}

#[derive(Serialize)]
struct FullHealthResponse {
    status: &'static str,
    mode: AppMode,
    revision: i64,
    build: String,
    degraded: Vec<String>,
    metrics: HealthMetricsResponse,
    torrent: TorrentHealthSnapshot,
}

#[derive(Serialize)]
struct HealthMetricsResponse {
    config_watch_latency_ms: i64,
    config_apply_latency_ms: i64,
    config_update_failures_total: u64,
    config_watch_slow_total: u64,
    guardrail_violations_total: u64,
    rate_limit_throttled_total: u64,
}

#[derive(Serialize)]
struct TorrentHealthSnapshot {
    active: i64,
    queue_depth: i64,
}

impl ApiServer {
    /// Construct a new API server with shared dependencies wired through application state.
    ///
    /// # Errors
    ///
    /// Returns an error if telemetry cannot be initialised or if persisting the `OpenAPI` document
    /// fails.
    #[allow(clippy::too_many_lines)]
    pub fn new(
        config: ConfigService,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
    ) -> Result<Self> {
        Self::with_config(Arc::new(config), events, torrent, telemetry)
    }

    #[allow(clippy::too_many_lines)]
    fn with_config(
        config: SharedConfig,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
    ) -> Result<Self> {
        let openapi_document = Arc::new(build_openapi_document());
        revaer_telemetry::persist_openapi(Path::new("docs/api/openapi.json"), &openapi_document)?;

        let telemetry_for_state = telemetry.clone();
        let state = Arc::new(ApiState::new(
            config,
            telemetry_for_state,
            openapi_document,
            events,
            torrent,
        ));

        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(move |request: &Request<_>| {
                let method = request.method().clone();
                let uri_path = request.uri().path();
                let request_id = request
                    .headers()
                    .get(HEADER_REQUEST_ID)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();

                let span = tracing::info_span!(
                    "http.request",
                    method = %method,
                    route = %uri_path,
                    request_id = tracing::field::Empty,
                    mode = tracing::field::Empty,
                    build_sha = %build_sha(),
                    status_code = tracing::field::Empty,
                    latency_ms = tracing::field::Empty
                );
                set_request_context(&span, request_id, uri_path.to_string());
                span
            })
            .on_request(|request: &Request<_>, span: &Span| {
                if let Some(matched) = request.extensions().get::<MatchedPath>() {
                    let request_id = request
                        .headers()
                        .get(HEADER_REQUEST_ID)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    set_request_context(span, request_id, matched.as_str().to_string());
                }
            })
            .on_response(|response: &Response, latency: Duration, span: &Span| {
                let status = response.status().as_u16();
                span.record("status_code", status);
                let latency_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX);
                span.record("latency_ms", latency_ms);
            });

        let layered = ServiceBuilder::new()
            .layer(revaer_telemetry::propagate_request_id_layer())
            .layer(revaer_telemetry::set_request_id_layer())
            .layer(trace_layer)
            .layer(HttpMetricsLayer::new(telemetry));

        let mut router = Router::new()
            .route("/health", get(health))
            .route("/health/full", get(health_full))
            .route("/.well-known/revaer.json", get(well_known))
            .route("/admin/setup/start", post(setup_start))
            .route(
                "/admin/setup/complete",
                post(setup_complete).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(
                    state.clone(), require_setup_token
                )),
            )
            .route(
                "/admin/settings",
                patch(settings_patch).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route(
                "/admin/torrents",
                get(list_torrents).post(create_torrent).route_layer(
                    middleware::from_fn_with_state::<
                        _,
                        Arc<ApiState>,
                        (State<Arc<ApiState>>, Request<Body>),
                    >(state.clone(), require_api_key),
                ),
            )
            .route(
                "/admin/torrents/:id",
                get(get_torrent).delete(delete_torrent).route_layer(
                    middleware::from_fn_with_state::<
                        _,
                        Arc<ApiState>,
                        (State<Arc<ApiState>>, Request<Body>),
                    >(state.clone(), require_api_key),
                ),
            )
            .route(
                "/v1/torrents",
                get(list_torrents).post(create_torrent).route_layer(
                    middleware::from_fn_with_state::<
                        _,
                        Arc<ApiState>,
                        (State<Arc<ApiState>>, Request<Body>),
                    >(state.clone(), require_api_key),
                ),
            )
            .route(
                "/v1/torrents/:id",
                get(get_torrent).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route(
                "/v1/torrents/:id/select",
                post(select_torrent).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route(
                "/v1/torrents/:id/action",
                post(action_torrent).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route(
                "/v1/events",
                get(stream_events).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route(
                "/v1/torrents/events",
                get(stream_events).route_layer(middleware::from_fn_with_state::<
                    _,
                    Arc<ApiState>,
                    (State<Arc<ApiState>>, Request<Body>),
                >(state.clone(), require_api_key)),
            )
            .route("/metrics", get(metrics))
            .route("/docs/openapi.json", get(openapi_document_handler));

        #[cfg(feature = "compat-qb")]
        {
            router = compat_qb::mount(router);
        }

        let router = router.route_layer(layered).with_state(state);

        Ok(Self { router })
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        info!("Starting API on {}", addr);
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, self.router.into_make_service()).await?;
        Ok(())
    }
}

async fn setup_start(
    State(state): State<Arc<ApiState>>,
    payload: Option<Json<SetupStartRequest>>,
) -> Result<Json<SetupStartResponse>, ApiError> {
    let payload = payload.map(|Json(p)| p).unwrap_or_default();

    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    if app.mode != AppMode::Setup {
        return Err(ApiError::conflict("system already configured"));
    }

    let ttl = payload
        .ttl_seconds
        .map_or(state.setup_token_ttl, Duration::from_secs);

    let issued_by = payload.issued_by.unwrap_or_else(|| "api".to_string());

    let token = state
        .config
        .issue_setup_token(ttl, &issued_by)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to issue setup token");
            ApiError::internal("failed to issue setup token")
        })?;

    Ok(Json(SetupStartResponse {
        token: token.plaintext,
        expires_at: token.expires_at,
    }))
}

async fn setup_complete(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    Json(mut changeset): Json<SettingsChangeset>,
) -> Result<Json<ConfigSnapshot>, ApiError> {
    let token = extract_setup_token(context)?;
    ensure_valid_setup_token(&state, &token).await?;
    coerce_app_profile_patch(&mut changeset)?;

    let snapshot = apply_setup_changes(&state, changeset, &token).await?;

    let _ = state.events.publish(CoreEvent::SettingsChanged {
        description: format!("setup_complete revision {}", snapshot.revision),
    });

    Ok(Json(snapshot))
}

fn extract_setup_token(context: AuthContext) -> Result<String, ApiError> {
    match context {
        AuthContext::SetupToken(token) => Ok(token),
        AuthContext::ApiKey { .. } => Err(ApiError::internal(
            "invalid authentication context for setup completion",
        )),
    }
}

async fn ensure_valid_setup_token(state: &ApiState, token: &str) -> Result<(), ApiError> {
    match state.config.validate_setup_token(token).await {
        Ok(()) => Ok(()),
        Err(err) => {
            warn!(error = %err, "setup token validation failed");
            Err(ApiError::unauthorized("invalid setup token"))
        }
    }
}

fn coerce_app_profile_patch(changeset: &mut SettingsChangeset) -> Result<(), ApiError> {
    let updated = match changeset.app_profile.take() {
        Some(Value::Object(mut map)) => {
            map.insert("mode".to_string(), json!("active"));
            Value::Object(map)
        }
        Some(Value::Null) | None => {
            let mut map = Map::new();
            map.insert("mode".to_string(), json!("active"));
            Value::Object(map)
        }
        Some(other) => {
            warn!("setup completion received invalid app_profile patch: {other:?}");
            return Err(ApiError::bad_request(
                "app_profile changeset must be a JSON object",
            ));
        }
    };
    changeset.app_profile = Some(updated);
    Ok(())
}

async fn apply_setup_changes(
    state: &ApiState,
    changeset: SettingsChangeset,
    token: &str,
) -> Result<ConfigSnapshot, ApiError> {
    state
        .config
        .apply_changeset("setup", "setup_complete", changeset)
        .await
        .map_err(|err| map_config_error(err, "failed to apply setup changes"))?;

    if let Err(err) = state.config.consume_setup_token(token).await {
        error!(error = %err, "failed to consume setup token after completion");
        return Err(ApiError::internal("failed to finalize setup"));
    }

    state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })
}

async fn settings_patch(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    Json(changeset): Json<SettingsChangeset>,
) -> Result<Json<ConfigSnapshot>, ApiError> {
    let key_id = match context {
        AuthContext::ApiKey { key_id } => key_id,
        AuthContext::SetupToken(_) => {
            return Err(ApiError::internal(
                "invalid authentication context for settings patch",
            ));
        }
    };

    state
        .config
        .apply_changeset(&key_id, "api_patch", changeset)
        .await
        .map_err(|err| map_config_error(err, "failed to apply settings changes"))?;

    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;

    let _ = state.events.publish(CoreEvent::SettingsChanged {
        description: format!("settings_patch revision {}", snapshot.revision),
    });

    Ok(Json(snapshot))
}

async fn create_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<TorrentCreateRequest>,
) -> Result<StatusCode, ApiError> {
    match context {
        AuthContext::ApiKey { .. } => {}
        AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    dispatch_torrent_add(state.torrent.as_ref(), &request).await?;
    state.set_metadata(
        request.id,
        TorrentMetadata::new(request.tags.clone(), request.trackers.clone()),
    );
    let torrent_name = request.name.as_deref().unwrap_or("<unspecified>");
    info!(torrent_id = %request.id, torrent_name = %torrent_name, "torrent submission requested");
    state.update_torrent_metrics().await;

    Ok(StatusCode::ACCEPTED)
}

async fn delete_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<StatusCode, ApiError> {
    match context {
        AuthContext::ApiKey { .. } => {}
        AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    dispatch_torrent_remove(state.torrent.as_ref(), id).await?;
    info!(torrent_id = %id, "torrent removal requested");
    state.remove_metadata(&id);
    state.update_torrent_metrics().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn select_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
    Json(request): Json<TorrentSelectionRequest>,
) -> Result<StatusCode, ApiError> {
    match context {
        AuthContext::ApiKey { .. } => {}
        AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let update: FileSelectionUpdate = request.into();
    handles
        .workflow()
        .update_selection(id, update)
        .await
        .map_err(|err| {
            error!(error = %err, torrent_id = %id, "failed to update torrent selection");
            ApiError::internal("failed to update torrent selection")
        })?;
    info!(torrent_id = %id, "torrent selection update requested");
    Ok(StatusCode::ACCEPTED)
}

async fn action_torrent(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    AxumPath(id): AxumPath<Uuid>,
    Json(action): Json<TorrentAction>,
) -> Result<StatusCode, ApiError> {
    match context {
        AuthContext::ApiKey { .. } => {}
        AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot manage torrents",
            ));
        }
    }

    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let workflow = handles.workflow();

    let result = match &action {
        TorrentAction::Pause => workflow.pause_torrent(id).await,
        TorrentAction::Resume => workflow.resume_torrent(id).await,
        TorrentAction::Remove { delete_data } => {
            let options = RemoveTorrent {
                with_data: *delete_data,
            };
            workflow.remove_torrent(id, options).await
        }
        TorrentAction::Reannounce => workflow.reannounce(id).await,
        TorrentAction::Recheck => workflow.recheck(id).await,
        TorrentAction::Sequential { enable } => workflow.set_sequential(id, *enable).await,
        TorrentAction::Rate {
            download_bps,
            upload_bps,
        } => {
            workflow
                .update_limits(
                    Some(id),
                    TorrentRateLimit {
                        download_bps: *download_bps,
                        upload_bps: *upload_bps,
                    },
                )
                .await
        }
    };

    result.map_err(|err| {
        error!(error = %err, torrent_id = %id, "torrent action failed");
        ApiError::internal("failed to execute torrent action")
    })?;

    if matches!(action, TorrentAction::Remove { .. }) {
        state.remove_metadata(&id);
    }
    info!(torrent_id = %id, action = ?action, "torrent action dispatched");
    Ok(StatusCode::ACCEPTED)
}

async fn fetch_all_torrents(handles: &TorrentHandles) -> Result<Vec<TorrentStatus>, ApiError> {
    handles.inspector().list().await.map_err(|err| {
        error!(error = %err, "failed to read torrent catalogue");
        ApiError::internal("failed to query torrent status")
    })
}

async fn fetch_torrent_status(
    handles: &TorrentHandles,
    id: Uuid,
) -> Result<TorrentStatus, ApiError> {
    handles
        .inspector()
        .get(id)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to load torrent status");
            ApiError::internal("failed to query torrent status")
        })?
        .ok_or_else(|| ApiError::not_found("torrent not found"))
}

#[allow(clippy::too_many_lines)]
async fn list_torrents(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<TorrentListQuery>,
) -> Result<Json<TorrentListResponse>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let statuses = fetch_all_torrents(handles).await?;
    state.record_torrent_metrics(&statuses);

    let state_filter = if let Some(filter) = query.state.as_deref() {
        Some(parse_state_filter(filter)?)
    } else {
        None
    };
    let tag_filters = query
        .tags
        .as_deref()
        .map(split_comma_separated)
        .unwrap_or_default();
    let tracker_filter = query.tracker.as_ref().map(|value| normalise_lower(value));
    let extension_filter = query
        .extension
        .as_ref()
        .map(|value| normalise_lower(value.trim_start_matches('.')));
    let name_filter = query.name.as_ref().map(|value| normalise_lower(value));

    let mut entries: Vec<StatusEntry> = statuses
        .into_iter()
        .map(|status| StatusEntry {
            metadata: state.get_metadata(&status.id),
            status,
        })
        .collect();

    entries.retain(|entry| {
        if let Some(expected) = state_filter {
            let current = TorrentStateKind::from(entry.status.state.clone());
            if current != expected {
                return false;
            }
        }

        if !tag_filters.is_empty() {
            let tags = entry
                .metadata
                .tags
                .iter()
                .map(|tag| tag.to_lowercase())
                .collect::<HashSet<_>>();
            if !tag_filters.iter().all(|filter| tags.contains(filter)) {
                return false;
            }
        }

        if let Some(ref tracker) = tracker_filter
            && !entry
                .metadata
                .trackers
                .iter()
                .any(|value| value.to_lowercase().contains(tracker))
        {
            return false;
        }

        if let Some(ref extension) = extension_filter {
            let matches_extension = entry.status.files.as_ref().is_some_and(|files| {
                files.iter().any(|file| {
                    file.path
                        .rsplit_once('.')
                        .is_some_and(|(_, ext)| normalise_lower(ext) == *extension)
                })
            });
            if !matches_extension {
                return false;
            }
        }

        if let Some(ref needle) = name_filter {
            let matched = entry
                .status
                .name
                .as_ref()
                .is_some_and(|name| name.to_lowercase().contains(needle));
            if !matched {
                return false;
            }
        }

        true
    });

    entries.sort_by(|a, b| {
        b.status
            .last_updated
            .cmp(&a.status.last_updated)
            .then_with(|| a.status.id.cmp(&b.status.id))
    });

    let cursor = if let Some(token) = query.cursor.as_ref() {
        Some(decode_cursor_token(token)?)
    } else {
        None
    };

    let mut start_index = 0;
    if let Some(cursor) = &cursor {
        while start_index < entries.len() {
            let status = &entries[start_index].status;
            if status.last_updated > cursor.last_updated
                || (status.last_updated == cursor.last_updated && status.id >= cursor.id)
            {
                start_index += 1;
            } else {
                break;
            }
        }
    }

    let limit = query
        .limit
        .map_or(DEFAULT_PAGE_SIZE, |value| value as usize)
        .clamp(1, MAX_PAGE_SIZE);
    let end_index = (start_index + limit).min(entries.len());
    let slice = &entries[start_index..end_index];

    let torrents: Vec<TorrentSummary> = slice
        .iter()
        .map(|entry| summary_from_components(entry.status.clone(), entry.metadata.clone()))
        .collect();

    let next = if end_index < entries.len() && !torrents.is_empty() {
        Some(encode_cursor_from_entry(&entries[end_index - 1])?)
    } else {
        None
    };

    Ok(Json(TorrentListResponse { torrents, next }))
}

async fn get_torrent(
    State(state): State<Arc<ApiState>>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<TorrentDetail>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let status = fetch_torrent_status(handles, id).await?;
    state.record_torrent_metrics(std::slice::from_ref(&status));
    let metadata = state.get_metadata(&status.id);
    Ok(Json(detail_from_components(status, metadata)))
}

async fn health(State(state): State<Arc<ApiState>>) -> Result<Json<HealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.remove_degraded_component("database");
            record_app_mode(snapshot.app_profile.mode.as_str());
            Ok(Json(HealthResponse {
                status: "ok",
                mode: snapshot.app_profile.mode.clone(),
                database: HealthComponent {
                    status: "ok",
                    revision: Some(snapshot.revision),
                },
            }))
        }
        Err(err) => {
            state.add_degraded_component("database");
            warn!(error = %err, "health check failed to reach database");
            Err(ApiError::service_unavailable(
                "database is currently unavailable",
            ))
        }
    }
}

async fn health_full(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FullHealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.remove_degraded_component("database");
            record_app_mode(snapshot.app_profile.mode.as_str());
            state.update_torrent_metrics().await;
            let metrics_snapshot = state.telemetry.snapshot();
            let metrics = HealthMetricsResponse {
                config_watch_latency_ms: metrics_snapshot.config_watch_latency_ms,
                config_apply_latency_ms: metrics_snapshot.config_apply_latency_ms,
                config_update_failures_total: metrics_snapshot.config_update_failures_total,
                config_watch_slow_total: metrics_snapshot.config_watch_slow_total,
                guardrail_violations_total: metrics_snapshot.guardrail_violations_total,
                rate_limit_throttled_total: metrics_snapshot.rate_limit_throttled_total,
            };
            let torrent = TorrentHealthSnapshot {
                active: metrics_snapshot.active_torrents,
                queue_depth: metrics_snapshot.queue_depth,
            };
            let degraded = state.current_health_degraded();
            let status = if degraded.is_empty() {
                "ok"
            } else {
                "degraded"
            };
            Ok(Json(FullHealthResponse {
                status,
                mode: snapshot.app_profile.mode.clone(),
                revision: snapshot.revision,
                build: build_sha().to_string(),
                degraded,
                metrics,
                torrent,
            }))
        }
        Err(err) => {
            state.add_degraded_component("database");
            warn!(error = %err, "full health check failed to reach database");
            Err(ApiError::service_unavailable(
                "database is currently unavailable",
            ))
        }
    }
}

async fn well_known(State(state): State<Arc<ApiState>>) -> Result<Json<ConfigSnapshot>, ApiError> {
    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;
    Ok(Json(snapshot))
}

async fn stream_events(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send>, ApiError>
{
    let last_id = headers
        .get(HEADER_LAST_EVENT_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<EventId>().ok());

    let filter = build_sse_filter(&query)?;
    let stream = event_sse_stream(state.events.clone(), last_id, filter);

    Ok(Sse::new(stream).keep_alive(
        sse::KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
            .text("keep-alive"),
    ))
}

async fn metrics(State(state): State<Arc<ApiState>>) -> Result<Response, ApiError> {
    match state.telemetry.render() {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/plain; version=0.0.4")
            .body(Body::from(body))
            .map_err(|err| {
                error!(error = %err, "failed to build metrics response");
                ApiError::internal("failed to build metrics response")
            }),
        Err(err) => {
            error!(error = %err, "failed to render metrics");
            Err(ApiError::internal("failed to render metrics"))
        }
    }
}

async fn openapi_document_handler(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json((*state.openapi_document).clone())
}

fn event_replay_stream(
    bus: EventBus,
    since: Option<EventId>,
) -> impl futures_core::Stream<Item = EventEnvelope> + Send {
    stream! {
        let mut stream = bus.subscribe(since);
        while let Some(envelope) = stream.next().await {
            yield envelope;
        }
    }
}

fn event_sse_stream(
    bus: EventBus,
    since: Option<EventId>,
    filter: SseFilter,
) -> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    let filter = Arc::new(filter);
    event_replay_stream(bus, since)
        .filter({
            let filter = Arc::clone(&filter);
            move |envelope| future::ready(matches_sse_filter(envelope, &filter))
        })
        .scan(None, move |last_id: &mut Option<EventId>, envelope| {
            if last_id.is_some_and(|prev| prev == envelope.id) {
                future::ready(Some(None))
            } else {
                *last_id = Some(envelope.id);
                future::ready(Some(Some(envelope)))
            }
        })
        .filter_map(|maybe| async move { maybe })
        .filter_map(|envelope| async move {
            match serde_json::to_string(&envelope) {
                Ok(payload) => Some(Ok(sse::Event::default()
                    .id(envelope.id.to_string())
                    .event(envelope.event.kind())
                    .data(payload))),
                Err(err) => {
                    error!(error = %err, "failed to serialise SSE event payload");
                    None
                }
            }
        })
}

#[allow(clippy::too_many_lines)]
fn build_openapi_document() -> Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Revaer Control Plane API",
            "version": "0.1.0"
        },
        "servers": [
            { "url": "http://localhost:7070" }
        ],
        "paths": {
            "/health": {
                "get": {
                    "summary": "Read the lightweight health probe",
                    "responses": {
                        "200": {
                            "description": "Health snapshot",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        },
                        "503": {
                            "description": "Service unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/health/full": {
                "get": {
                    "summary": "Read the extended health probe",
                    "responses": {
                        "200": {
                            "description": "Detailed health snapshot",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/FullHealthResponse" }
                                }
                            }
                        },
                        "503": {
                            "description": "Service unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/.well-known/revaer.json": {
                "get": {
                    "summary": "Retrieve the configuration snapshot exposed to clients",
                    "responses": {
                        "200": {
                            "description": "Configuration document",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ConfigSnapshot" }
                                }
                            }
                        }
                    }
                }
            },
            "/admin/setup/start": {
                "post": {
                    "summary": "Issue a one-time setup token",
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/SetupStartRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Setup token issued",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/SetupStartResponse" }
                                }
                            }
                        },
                        "409": {
                            "description": "System already configured",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/admin/setup/complete": {
                "post": {
                    "summary": "Complete initial setup and persist configuration",
                    "security": [ { "SetupToken": [] } ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/SettingsChangeset" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Snapshot after setup",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ConfigSnapshot" }
                                }
                            }
                        },
                        "401": {
                            "description": "Invalid setup token",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/admin/settings": {
                "patch": {
                    "summary": "Apply configuration mutations",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/SettingsChangeset" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Updated configuration snapshot",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ConfigSnapshot" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "422": {
                            "description": "Validation error",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
    "/v1/torrents": {
                "get": {
                    "summary": "List torrents with pagination and filters",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        { "name": "limit", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 200 } },
                        { "name": "cursor", "in": "query", "schema": { "type": "string" } },
                        { "name": "state", "in": "query", "schema": { "type": "string", "enum": ["queued", "fetching_metadata", "downloading", "seeding", "completed", "failed", "stopped"] } },
                        { "name": "tracker", "in": "query", "schema": { "type": "string" } },
                        { "name": "extension", "in": "query", "schema": { "type": "string" } },
                        { "name": "tags", "in": "query", "schema": { "type": "string" }, "description": "Comma separated list of tags" },
                        { "name": "name", "in": "query", "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Torrent collection",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TorrentListResponse" }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid filters",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Torrent workflow unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                },
                "post": {
                    "summary": "Submit a torrent descriptor to the engine",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TorrentCreateRequest" }
                            }
                        }
                    },
                    "responses": {
                        "202": {
                            "description": "Torrent accepted"
                        },
                        "400": {
                            "description": "Invalid submission",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Torrent workflow unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/v1/torrents/{id}": {
                "get": {
                    "summary": "Fetch torrent detail by identifier",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "Torrent detail",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TorrentDetail" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "404": {
                            "description": "Torrent not found",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Torrent workflow unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/v1/torrents/{id}/select": {
                "post": {
                    "summary": "Update a torrent's file selection",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TorrentSelectionRequest" }
                            }
                        }
                    },
                    "responses": {
                        "202": { "description": "Selection update accepted" },
                        "400": {
                            "description": "Invalid selection payload",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Torrent workflow unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/v1/torrents/{id}/action": {
                "post": {
                    "summary": "Trigger a torrent control action",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/TorrentAction" }
                            }
                        }
                    },
                    "responses": {
                        "202": { "description": "Action accepted" },
                        "400": {
                            "description": "Invalid action payload",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Torrent workflow unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/v1/torrents/events": {
                "get": {
                    "summary": "Subscribe to torrent events via SSE",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        { "name": "torrent", "in": "query", "schema": { "type": "string" }, "description": "Comma separated torrent identifiers" },
                        { "name": "event", "in": "query", "schema": { "type": "string" }, "description": "Comma separated event kinds" },
                        { "name": "state", "in": "query", "schema": { "type": "string" }, "description": "Filter state change events by new state" }
                    ],
                    "responses": {
                        "200": {
                            "description": "SSE stream",
                            "content": {
                                "text/event-stream": {
                                    "schema": { "type": "string" }
                                }
                            }
                        },
                        "401": {
                            "description": "Authentication failed",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "429": {
                            "description": "Rate limit exceeded",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        },
                        "503": {
                            "description": "Event stream unavailable",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ProblemDetails" }
                                }
                            }
                        }
                    }
                }
            },
            "/metrics": {
                "get": {
                    "summary": "Expose Prometheus metrics",
                    "responses": {
                        "200": {
                            "description": "Prometheus metrics",
                            "content": {
                                "text/plain": {
                                    "schema": { "type": "string" }
                                }
                            }
                        }
                    }
                }
            },
            "/docs/openapi.json": {
                "get": {
                    "summary": "Serve the generated OpenAPI specification",
                    "responses": {
                        "200": {
                            "description": "OpenAPI document",
                            "content": {
                                "application/json": {
                                    "schema": { "type": "object" }
                                }
                            }
                        }
                    }
                }
            }
        },
        "components": {
            "securitySchemes": {
                "SetupToken": {
                    "type": "apiKey",
                    "name": HEADER_SETUP_TOKEN,
                    "in": "header"
                },
                "ApiKeyAuth": {
                    "type": "apiKey",
                    "name": HEADER_API_KEY,
                    "in": "header"
                }
            },
            "schemas": {
                "ProblemDetails": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string" },
                        "title": { "type": "string" },
                        "status": { "type": "integer" },
                        "detail": { "type": "string" }
                    },
                    "required": ["type", "title", "status"]
                },
                "SetupStartRequest": {
                    "type": "object",
                    "properties": {
                        "issued_by": { "type": "string" },
                        "ttl_seconds": { "type": "integer", "format": "int64" }
                    }
                },
                "SetupStartResponse": {
                    "type": "object",
                    "properties": {
                        "token": { "type": "string" },
                        "expires_at": { "type": "string", "format": "date-time" }
                    },
                    "required": ["token", "expires_at"]
                },
                "HealthComponent": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "revision": { "type": "integer", "format": "int64" }
                    },
                    "required": ["status"]
                },
                "HealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "mode": { "type": "string" },
                        "database": { "$ref": "#/components/schemas/HealthComponent" }
                    },
                    "required": ["status", "mode", "database"]
                },
                "ConfigSnapshot": {
                    "type": "object",
                    "properties": {
                        "revision": { "type": "integer", "format": "int64" },
                        "app_profile": { "type": "object" },
                        "engine_profile": { "type": "object" },
                        "fs_policy": { "type": "object" }
                    },
                    "required": ["revision", "app_profile", "engine_profile", "fs_policy"]
                },
                "SettingsChangeset": {
                    "type": "object",
                    "properties": {
                        "app_profile": { "type": ["object", "null"] },
                        "engine_profile": { "type": ["object", "null"] },
                        "fs_policy": { "type": ["object", "null"] },
                        "api_keys": { "type": "array", "items": { "type": "object" } },
                        "secrets": { "type": "array", "items": { "type": "object" } }
                    }
                },
                "TorrentCreateRequest": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "magnet": { "type": ["string", "null"] },
                        "metainfo": { "type": ["string", "null"], "format": "byte" },
                        "name": { "type": ["string", "null"] },
                        "download_dir": { "type": ["string", "null"] },
                        "sequential": { "type": ["boolean", "null"] },
                        "include": { "type": "array", "items": { "type": "string" } },
                        "exclude": { "type": "array", "items": { "type": "string" } },
                        "skip_fluff": { "type": "boolean" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "trackers": { "type": "array", "items": { "type": "string" } },
                        "max_download_bps": { "type": ["integer", "null"], "format": "int64" },
                        "max_upload_bps": { "type": ["integer", "null"], "format": "int64" }
                    },
                    "required": ["id"]
                },
                "FilePriorityOverride": {
                    "type": "object",
                    "properties": {
                        "index": { "type": "integer", "format": "int32" },
                        "priority": { "type": "string", "enum": ["skip", "low", "normal", "high"] }
                    },
                    "required": ["index", "priority"]
                },
                "TorrentSelectionRequest": {
                    "type": "object",
                    "properties": {
                        "include": { "type": "array", "items": { "type": "string" } },
                        "exclude": { "type": "array", "items": { "type": "string" } },
                        "skip_fluff": { "type": "boolean" },
                        "priorities": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/FilePriorityOverride" }
                        }
                    }
                },
                "TorrentStateView": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": [
                                "queued",
                                "fetching_metadata",
                                "downloading",
                                "seeding",
                                "completed",
                                "failed",
                                "stopped"
                            ]
                        },
                        "failure_message": { "type": ["string", "null"] }
                    },
                    "required": ["kind"]
                },
                "TorrentProgressView": {
                    "type": "object",
                    "properties": {
                        "bytes_downloaded": { "type": "integer", "format": "int64" },
                        "bytes_total": { "type": "integer", "format": "int64" },
                        "percent_complete": { "type": "number", "format": "float" },
                        "eta_seconds": { "type": ["integer", "null"], "format": "int64" }
                    },
                    "required": ["bytes_downloaded", "bytes_total", "percent_complete"]
                },
                "TorrentRatesView": {
                    "type": "object",
                    "properties": {
                        "download_bps": { "type": "integer", "format": "int64" },
                        "upload_bps": { "type": "integer", "format": "int64" },
                        "ratio": { "type": "number", "format": "float" }
                    },
                    "required": ["download_bps", "upload_bps", "ratio"]
                },
                "TorrentFileView": {
                    "type": "object",
                    "properties": {
                        "index": { "type": "integer", "format": "int32" },
                        "path": { "type": "string" },
                        "size_bytes": { "type": "integer", "format": "int64" },
                        "bytes_completed": { "type": "integer", "format": "int64" },
                        "priority": { "type": "string", "enum": ["skip", "low", "normal", "high"] },
                        "selected": { "type": "boolean" }
                    },
                    "required": ["index", "path", "size_bytes", "bytes_completed", "priority", "selected"]
                },
                "TorrentSummary": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "name": { "type": ["string", "null"] },
                        "state": { "$ref": "#/components/schemas/TorrentStateView" },
                        "progress": { "$ref": "#/components/schemas/TorrentProgressView" },
                        "rates": { "$ref": "#/components/schemas/TorrentRatesView" },
                        "library_path": { "type": ["string", "null"] },
                        "download_dir": { "type": ["string", "null"] },
                        "sequential": { "type": "boolean" },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "trackers": { "type": "array", "items": { "type": "string" } },
                        "added_at": { "type": "string", "format": "date-time" },
                        "completed_at": { "type": ["string", "null"], "format": "date-time" },
                        "last_updated": { "type": "string", "format": "date-time" }
                    },
                    "required": ["id", "state", "progress", "rates", "sequential", "tags", "trackers", "added_at", "last_updated"]
                },
                "TorrentDetail": {
                    "type": "object",
                    "properties": {
                        "summary": { "$ref": "#/components/schemas/TorrentSummary" },
                        "files": {
                            "type": ["array", "null"],
                            "items": { "$ref": "#/components/schemas/TorrentFileView" }
                        }
                    },
                    "required": ["summary"]
                },
                "TorrentListResponse": {
                    "type": "object",
                    "properties": {
                        "torrents": {
                            "type": "array",
                            "items": { "$ref": "#/components/schemas/TorrentSummary" }
                        },
                        "next": { "type": ["string", "null"] }
                    },
                    "required": ["torrents"]
                },
                "TorrentAction": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": [
                                "pause",
                                "resume",
                                "remove",
                                "reannounce",
                                "recheck",
                                "sequential",
                                "rate"
                            ]
                        },
                        "delete_data": { "type": ["boolean", "null"] },
                        "enable": { "type": ["boolean", "null"] },
                        "download_bps": { "type": ["integer", "null"], "format": "int64" },
                        "upload_bps": { "type": ["integer", "null"], "format": "int64" }
                    },
                    "required": ["type"]
                },
                "HealthMetricsResponse": {
                    "type": "object",
                    "properties": {
                        "config_watch_latency_ms": { "type": "integer", "format": "int64" },
                        "config_apply_latency_ms": { "type": "integer", "format": "int64" },
                        "config_update_failures_total": { "type": "integer", "format": "int64" },
                        "config_watch_slow_total": { "type": "integer", "format": "int64" },
                        "guardrail_violations_total": { "type": "integer", "format": "int64" },
                        "rate_limit_throttled_total": { "type": "integer", "format": "int64" }
                    },
                    "required": [
                        "config_watch_latency_ms",
                        "config_apply_latency_ms",
                        "config_update_failures_total",
                        "config_watch_slow_total",
                        "guardrail_violations_total",
                        "rate_limit_throttled_total"
                    ]
                },
                "TorrentHealthSnapshot": {
                    "type": "object",
                    "properties": {
                        "active": { "type": "integer", "format": "int64" },
                        "queue_depth": { "type": "integer", "format": "int64" }
                    },
                    "required": ["active", "queue_depth"]
                },
                "FullHealthResponse": {
                    "type": "object",
                    "properties": {
                        "status": { "type": "string" },
                        "mode": { "type": "string" },
                        "revision": { "type": "integer", "format": "int64" },
                        "build": { "type": "string" },
                        "degraded": { "type": "array", "items": { "type": "string" } },
                        "metrics": { "$ref": "#/components/schemas/HealthMetricsResponse" },
                        "torrent": { "$ref": "#/components/schemas/TorrentHealthSnapshot" }
                    },
                    "required": ["status", "mode", "revision", "build", "degraded", "metrics", "torrent"]
                }
            }
        }
    })
}

#[must_use]
pub fn openapi_document() -> Value {
    build_openapi_document()
}

async fn require_setup_token(
    State(state): State<Arc<ApiState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    if app.mode != AppMode::Setup {
        return Err(ApiError::setup_required(
            "system is not accepting setup requests",
        ));
    }

    let header_value = req
        .headers()
        .get(HEADER_SETUP_TOKEN)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("missing setup token"))?;
    let token = header_value
        .to_str()
        .map_err(|_| ApiError::bad_request("setup token header must be valid UTF-8"))?
        .trim()
        .to_string();

    req.extensions_mut().insert(AuthContext::SetupToken(token));

    Ok(next.run(req).await)
}

async fn require_api_key(
    State(state): State<Arc<ApiState>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    if app.mode != AppMode::Active {
        return Err(ApiError::setup_required("system is still in setup mode"));
    }

    let header_value = req
        .headers()
        .get(HEADER_API_KEY)
        .cloned()
        .ok_or_else(|| ApiError::unauthorized("missing API key header"))?;
    let header_value = header_value
        .to_str()
        .map_err(|_| ApiError::bad_request("API key header must be valid UTF-8"))?;

    let (key_id, secret) = header_value
        .split_once(':')
        .ok_or_else(|| ApiError::unauthorized("API key must be provided as key_id:secret"))?;

    let auth = state
        .config
        .authenticate_api_key(key_id, secret)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to verify API key");
            ApiError::internal("failed to verify API key")
        })?;

    let Some(auth) = auth else {
        return Err(ApiError::unauthorized("invalid API key"));
    };

    state.enforce_rate_limit(&auth.key_id, auth.rate_limit.as_ref())?;

    req.extensions_mut().insert(AuthContext::ApiKey {
        key_id: auth.key_id,
    });

    Ok(next.run(req).await)
}

fn map_config_error(err: anyhow::Error, context: &'static str) -> ApiError {
    match err.downcast::<ConfigError>() {
        Ok(config_err) => {
            warn!(error = %config_err, "{context}");
            let mut api_error = ApiError::config_invalid(config_err.to_string());
            let params = invalid_params_for_config_error(&config_err);
            if !params.is_empty() {
                api_error = api_error.with_invalid_params(params);
            }
            api_error
        }
        Err(other) => {
            error!(error = %other, "{context}");
            ApiError::internal(context)
        }
    }
}

fn invalid_params_for_config_error(error: &ConfigError) -> Vec<ProblemInvalidParam> {
    match error {
        ConfigError::ImmutableField { section, field } => vec![ProblemInvalidParam {
            pointer: pointer_for(section, field),
            message: format!("field '{field}' in '{section}' is immutable"),
        }],
        ConfigError::InvalidField {
            section,
            field,
            message,
        } => vec![ProblemInvalidParam {
            pointer: pointer_for(section, field),
            message: message.clone(),
        }],
        ConfigError::UnknownField { section, field } => vec![ProblemInvalidParam {
            pointer: pointer_for(section, field),
            message: format!("unknown field '{field}' in '{section}'"),
        }],
    }
}

fn pointer_for(section: &str, field: &str) -> String {
    let mut pointer = String::new();
    pointer.push('/');
    pointer.push_str(&encode_pointer_segment(section));

    if field != "<root>" && !field.is_empty() {
        pointer.push('/');
        pointer.push_str(&encode_pointer_segment(field));
    }

    pointer
}

fn encode_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

pub(crate) async fn dispatch_torrent_add(
    handles: Option<&TorrentHandles>,
    request: &TorrentCreateRequest,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    let add_request = build_add_torrent(request)?;

    handles
        .workflow()
        .add_torrent(add_request)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to add torrent through workflow");
            ApiError::internal("failed to add torrent")
        })
}

pub(crate) async fn dispatch_torrent_remove(
    handles: Option<&TorrentHandles>,
    id: Uuid,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    handles
        .workflow()
        .remove_torrent(id, RemoveTorrent::default())
        .await
        .map_err(|err| {
            error!(error = %err, "failed to remove torrent through workflow");
            ApiError::internal("failed to remove torrent")
        })
}

pub(crate) fn build_add_torrent(request: &TorrentCreateRequest) -> Result<AddTorrent, ApiError> {
    let magnet = request
        .magnet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let source = if let Some(magnet) = magnet {
        TorrentSource::magnet(magnet.to_string())
    } else if let Some(encoded) = &request.metainfo {
        let bytes = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| ApiError::bad_request("metainfo payload must be base64 encoded"))?;
        if bytes.len() > MAX_METAINFO_BYTES {
            return Err(ApiError::bad_request(
                "metainfo payload exceeds the 5 MiB limit",
            ));
        }
        TorrentSource::metainfo(bytes)
    } else {
        return Err(ApiError::bad_request(
            "either magnet or metainfo payload must be provided",
        ));
    };

    let options = request.to_options();

    Ok(AddTorrent {
        id: request.id,
        source,
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{ConfigFacade, SharedConfig};
    use async_trait::async_trait;
    use axum::{
        extract::{Form, Query, State},
        http::StatusCode,
    };
    use chrono::{Duration as ChronoDuration, Utc};
    use futures_util::{StreamExt, pin_mut};
    use revaer_config::{ConfigError, EngineProfile, FsPolicy};
    use revaer_torrent_core::{
        AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentInspector, TorrentProgress,
        TorrentRateLimit, TorrentRates, TorrentSource, TorrentStatus, TorrentWorkflow,
    };
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::{Mutex, oneshot};
    use tokio::time::{sleep, timeout};
    use uuid::Uuid;

    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[cfg(feature = "compat-qb")]
    use crate::compat_qb::{
        self, SyncParams, TorrentAddForm, TorrentHashesForm, TorrentsInfoParams, TransferLimitForm,
    };

    #[derive(Clone)]
    struct MockConfig {
        inner: Arc<tokio::sync::Mutex<MockConfigState>>,
        fail_snapshot: Arc<AtomicBool>,
    }

    struct MockConfigState {
        snapshot: ConfigSnapshot,
        tokens: HashMap<String, DateTime<Utc>>,
        api_keys: HashMap<String, MockApiKey>,
    }

    struct MockApiKey {
        secret: String,
        auth: ApiKeyAuth,
        enabled: bool,
    }

    impl MockConfig {
        fn new() -> Self {
            let snapshot = ConfigSnapshot {
                revision: 1,
                app_profile: AppProfile {
                    id: Uuid::new_v4(),
                    instance_name: "revaer".to_string(),
                    mode: AppMode::Setup,
                    version: 1,
                    http_port: 7070,
                    bind_addr: IpAddr::from_str("127.0.0.1").expect("ip"),
                    telemetry: Value::Null,
                    features: Value::Null,
                    immutable_keys: Value::Null,
                },
                engine_profile: EngineProfile {
                    id: Uuid::new_v4(),
                    implementation: "stub".to_string(),
                    listen_port: Some(6881),
                    dht: true,
                    encryption: "preferred".to_string(),
                    max_active: Some(10),
                    max_download_bps: None,
                    max_upload_bps: None,
                    sequential_default: false,
                    resume_dir: "/tmp/resume".to_string(),
                    download_root: "/tmp/downloads".to_string(),
                    tracker: Value::Null,
                },
                fs_policy: FsPolicy {
                    id: Uuid::new_v4(),
                    library_root: "/tmp/library".to_string(),
                    extract: false,
                    par2: "disabled".to_string(),
                    flatten: false,
                    move_mode: "copy".to_string(),
                    cleanup_keep: Value::Null,
                    cleanup_drop: Value::Null,
                    chmod_file: None,
                    chmod_dir: None,
                    owner: None,
                    group: None,
                    umask: None,
                    allow_paths: Value::Array(vec![]),
                },
            };
            Self {
                inner: Arc::new(tokio::sync::Mutex::new(MockConfigState {
                    snapshot,
                    tokens: HashMap::new(),
                    api_keys: HashMap::new(),
                })),
                fail_snapshot: Arc::new(AtomicBool::new(false)),
            }
        }

        fn shared(&self) -> SharedConfig {
            Arc::new(self.clone()) as SharedConfig
        }

        async fn set_app_mode(&self, mode: AppMode) {
            let mut guard = self.inner.lock().await;
            guard.snapshot.app_profile.mode = mode;
        }

        async fn insert_api_key(&self, key_id: &str, secret: &str) {
            let mut guard = self.inner.lock().await;
            guard.api_keys.insert(
                key_id.to_string(),
                MockApiKey {
                    secret: secret.to_string(),
                    auth: ApiKeyAuth {
                        key_id: key_id.to_string(),
                        label: Some("test".to_string()),
                        rate_limit: None,
                    },
                    enabled: true,
                },
            );
        }

        fn set_fail_snapshot(&self, flag: bool) {
            self.fail_snapshot.store(flag, Ordering::SeqCst);
        }

        async fn snapshot(&self) -> ConfigSnapshot {
            self.inner.lock().await.snapshot.clone()
        }
    }

    #[async_trait]
    impl ConfigFacade for MockConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(self.inner.lock().await.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(&self, ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            let mut guard = self.inner.lock().await;
            let token = format!("token-{}", guard.tokens.len() + 1);
            let expires_at = Utc::now() + ChronoDuration::from_std(ttl).expect("ttl");
            guard.tokens.insert(token.clone(), expires_at);
            drop(guard);
            Ok(SetupToken {
                plaintext: token,
                expires_at,
            })
        }

        async fn validate_setup_token(&self, token: &str) -> Result<()> {
            let expires = {
                let guard = self.inner.lock().await;
                guard
                    .tokens
                    .get(token)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("unknown token"))?
            };
            if expires > Utc::now() {
                Ok(())
            } else {
                anyhow::bail!("expired")
            }
        }

        async fn consume_setup_token(&self, token: &str) -> Result<()> {
            {
                let mut guard = self.inner.lock().await;
                guard
                    .tokens
                    .remove(token)
                    .ok_or_else(|| anyhow::anyhow!("unknown token"))?;
            }
            Ok(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            mut changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            let mut guard = self.inner.lock().await;
            let mut app_changed = false;
            let mut engine_changed = false;
            let mut fs_changed = false;

            if let Some(patch) = changeset.app_profile.take() {
                apply_app_profile_patch(&mut guard.snapshot.app_profile, &patch)?;
                app_changed = true;
            }

            if let Some(patch) = changeset.engine_profile.take() {
                apply_engine_patch(&mut guard.snapshot.engine_profile, &patch);
                engine_changed = true;
            }

            if let Some(patch) = changeset.fs_policy.take() {
                apply_fs_patch(&mut guard.snapshot.fs_policy, &patch);
                fs_changed = true;
            }

            for patch in changeset.api_keys {
                match patch {
                    revaer_config::ApiKeyPatch::Upsert {
                        key_id,
                        secret,
                        enabled,
                        label,
                        ..
                    } => {
                        let secret = secret.unwrap_or_else(|| "secret".to_string());
                        guard.api_keys.insert(
                            key_id.clone(),
                            MockApiKey {
                                secret,
                                auth: ApiKeyAuth {
                                    key_id,
                                    label,
                                    rate_limit: None,
                                },
                                enabled: enabled.unwrap_or(true),
                            },
                        );
                    }
                    revaer_config::ApiKeyPatch::Delete { key_id } => {
                        guard.api_keys.remove(&key_id);
                    }
                }
            }

            guard.snapshot.revision += 1;
            Ok(AppliedChanges {
                revision: guard.snapshot.revision,
                app_profile: app_changed.then(|| guard.snapshot.app_profile.clone()),
                engine_profile: engine_changed.then(|| guard.snapshot.engine_profile.clone()),
                fs_policy: fs_changed.then(|| guard.snapshot.fs_policy.clone()),
            })
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            if self.fail_snapshot.load(Ordering::SeqCst) {
                anyhow::bail!("snapshot unavailable")
            }
            Ok(self.inner.lock().await.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            key_id: &str,
            secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            let auth = {
                let guard = self.inner.lock().await;
                guard.api_keys.get(key_id).and_then(|entry| {
                    (entry.enabled && entry.secret == secret).then(|| entry.auth.clone())
                })
            };
            Ok(auth)
        }
    }

    fn apply_app_profile_patch(profile: &mut AppProfile, patch: &Value) -> Result<()> {
        if let Some(name) = patch.get("instance_name").and_then(Value::as_str) {
            profile.instance_name = name.to_string();
        }
        if let Some(bind) = patch.get("bind_addr").and_then(Value::as_str) {
            profile.bind_addr = IpAddr::from_str(bind)?;
        }
        if let Some(port) = patch.get("http_port").and_then(Value::as_i64) {
            profile.http_port = i32::try_from(port)
                .map_err(|_| anyhow::anyhow!("http_port {port} exceeds i32 range"))?;
        }
        if let Some(mode) = patch.get("mode").and_then(Value::as_str) {
            profile.mode = mode.parse()?;
        }
        Ok(())
    }

    fn apply_engine_patch(profile: &mut EngineProfile, patch: &Value) {
        if let Some(impl_name) = patch.get("implementation").and_then(Value::as_str) {
            profile.implementation = impl_name.to_string();
        }
        if let Some(resume) = patch.get("resume_dir").and_then(Value::as_str) {
            profile.resume_dir = resume.to_string();
        }
        if let Some(download) = patch.get("download_root").and_then(Value::as_str) {
            profile.download_root = download.to_string();
        }
    }

    fn apply_fs_patch(policy: &mut FsPolicy, patch: &Value) {
        if let Some(root) = patch.get("library_root").and_then(Value::as_str) {
            policy.library_root = root.to_string();
        }
        if let Some(allow_paths) = patch.get("allow_paths") {
            policy.allow_paths = allow_paths.clone();
        }
    }

    #[derive(Default)]
    struct StubTorrent {
        statuses: Mutex<Vec<TorrentStatus>>,
        added: Mutex<Vec<AddTorrent>>,
        removed: Mutex<Vec<Uuid>>,
        selections: Mutex<Vec<(Uuid, FileSelectionUpdate)>>,
        actions: Mutex<Vec<(Uuid, String)>>,
    }

    impl StubTorrent {
        async fn push_status(&self, status: TorrentStatus) {
            self.statuses.lock().await.push(status);
        }

        async fn added(&self) -> Vec<AddTorrent> {
            self.added.lock().await.clone()
        }

        async fn selections(&self) -> Vec<(Uuid, FileSelectionUpdate)> {
            self.selections.lock().await.clone()
        }

        async fn actions(&self) -> Vec<(Uuid, String)> {
            self.actions.lock().await.clone()
        }
    }

    #[async_trait]
    impl TorrentWorkflow for StubTorrent {
        async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
            self.added.lock().await.push(request.clone());
            let status = TorrentStatus {
                id: request.id,
                name: request.options.name_hint.clone(),
                progress: TorrentProgress {
                    bytes_total: 100,
                    bytes_downloaded: 0,
                    ..TorrentProgress::default()
                },
                last_updated: Utc::now(),
                ..TorrentStatus::default()
            };
            self.statuses.lock().await.push(status);
            Ok(())
        }

        async fn remove_torrent(&self, id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            self.removed.lock().await.push(id);
            self.statuses.lock().await.retain(|status| status.id != id);
            self.actions.lock().await.push((id, "remove".to_string()));
            Ok(())
        }

        async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "pause".to_string()));
            Ok(())
        }

        async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "resume".to_string()));
            Ok(())
        }

        async fn update_selection(
            &self,
            id: Uuid,
            rules: FileSelectionUpdate,
        ) -> anyhow::Result<()> {
            self.selections.lock().await.push((id, rules));
            Ok(())
        }

        async fn set_sequential(&self, id: Uuid, enable: bool) -> anyhow::Result<()> {
            self.actions
                .lock()
                .await
                .push((id, format!("sequential:{enable}")));
            Ok(())
        }

        async fn update_limits(
            &self,
            id: Option<Uuid>,
            limits: TorrentRateLimit,
        ) -> anyhow::Result<()> {
            if let Some(id) = id {
                self.actions.lock().await.push((id, "rate".to_string()));
            }
            let _ = limits;
            Ok(())
        }

        async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions
                .lock()
                .await
                .push((id, "reannounce".to_string()));
            Ok(())
        }

        async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
            self.actions.lock().await.push((id, "recheck".to_string()));
            Ok(())
        }
    }

    #[async_trait]
    impl TorrentInspector for StubTorrent {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(self.statuses.lock().await.clone())
        }

        async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(self
                .statuses
                .lock()
                .await
                .iter()
                .find(|status| status.id == id)
                .cloned())
        }
    }

    #[tokio::test]
    async fn setup_flow_promotes_active_mode() -> Result<()> {
        let config = MockConfig::new();
        let events = EventBus::with_capacity(32);
        let mut event_stream = events.subscribe(None);
        let metrics = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events.clone(),
            None,
        ));

        let Json(start) = setup_start(State(state.clone()), None)
            .await
            .expect("setup start");
        assert!(!start.token.is_empty());

        let changeset = SettingsChangeset {
            app_profile: Some(json!({
                "instance_name": "demo",
                "bind_addr": "127.0.0.1",
                "http_port": 8080,
                "mode": "active"
            })),
            engine_profile: Some(json!({
                "implementation": "libtorrent",
                "resume_dir": "/var/lib/revaer/resume",
                "download_root": "/var/lib/revaer/downloads"
            })),
            fs_policy: Some(json!({
                "library_root": "/data/library",
                "allow_paths": ["/data"]
            })),
            api_keys: vec![revaer_config::ApiKeyPatch::Upsert {
                key_id: "bootstrap".to_string(),
                label: Some("bootstrap".to_string()),
                enabled: Some(true),
                secret: Some("secret".to_string()),
                rate_limit: None,
            }],
            secrets: vec![],
        };

        let Json(snapshot) = setup_complete(
            State(state.clone()),
            Extension(AuthContext::SetupToken(start.token.clone())),
            Json(changeset),
        )
        .await
        .expect("setup complete");

        assert_eq!(snapshot.app_profile.mode, AppMode::Active);
        let event = timeout(Duration::from_secs(1), event_stream.next())
            .await
            .expect("settings event")
            .expect("event value");
        assert!(matches!(event.event, CoreEvent::SettingsChanged { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn settings_patch_updates_snapshot() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        config.insert_api_key("admin", "secret").await;

        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            None,
        ));

        let changeset = SettingsChangeset {
            app_profile: Some(json!({"instance_name": "patched"})),
            engine_profile: None,
            fs_policy: None,
            api_keys: Vec::new(),
            secrets: Vec::new(),
        };

        let Json(response) = settings_patch(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "admin".to_string(),
            }),
            Json(changeset),
        )
        .await
        .expect("settings patch");

        assert_eq!(response.app_profile.instance_name, "patched");
        let snapshot = config.snapshot().await;
        assert_eq!(snapshot.app_profile.instance_name, "patched");
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn torrent_endpoints_execute_workflow() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        config.insert_api_key("operator", "secret").await;

        let events = EventBus::with_capacity(32);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let workflow: Arc<dyn TorrentWorkflow> = stub.clone();
        let inspector: Arc<dyn TorrentInspector> = stub.clone();
        let handles = TorrentHandles::new(workflow, inspector);
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let existing_id = Uuid::new_v4();
        let status = TorrentStatus {
            id: existing_id,
            name: Some("existing".to_string()),
            progress: TorrentProgress {
                bytes_total: 100,
                bytes_downloaded: 100,
                ..TorrentProgress::default()
            },
            state: TorrentState::Completed,
            library_path: Some("/library/existing".to_string()),
            ..TorrentStatus::default()
        };
        stub.push_status(status).await;

        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some("magnet:?xt=urn:btih:test".to_string()),
            name: Some("example".to_string()),
            ..TorrentCreateRequest::default()
        };

        create_torrent(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "operator".to_string(),
            }),
            Json(request.clone()),
        )
        .await
        .expect("create torrent");

        assert_eq!(stub.added().await.len(), 1);

        let Json(list) = list_torrents(State(state.clone()), Query(TorrentListQuery::default()))
            .await
            .expect("list torrents");
        assert!(list.torrents.iter().any(|item| item.id == existing_id));

        let Json(detail) = get_torrent(State(state.clone()), AxumPath(existing_id))
            .await
            .expect("get torrent");
        assert_eq!(detail.summary.id, existing_id);

        let selection = TorrentSelectionRequest {
            include: vec!["*.mkv".to_string()],
            exclude: vec![],
            skip_fluff: Some(true),
            priorities: Vec::new(),
        };
        select_torrent(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "operator".to_string(),
            }),
            AxumPath(existing_id),
            Json(selection.clone()),
        )
        .await
        .expect("select torrent");
        assert_eq!(stub.selections().await.len(), 1);

        action_torrent(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "operator".to_string(),
            }),
            AxumPath(existing_id),
            Json(TorrentAction::Sequential { enable: true }),
        )
        .await
        .expect("sequential action");

        action_torrent(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "operator".to_string(),
            }),
            AxumPath(existing_id),
            Json(TorrentAction::Remove { delete_data: true }),
        )
        .await
        .expect("remove action");
        assert!(
            stub.actions()
                .await
                .iter()
                .any(|(_, action)| action == "remove")
        );

        delete_torrent(
            State(state.clone()),
            Extension(AuthContext::ApiKey {
                key_id: "operator".to_string(),
            }),
            AxumPath(request.id),
        )
        .await
        .expect("delete torrent");
        Ok(())
    }

    #[tokio::test]
    async fn health_endpoints_reflect_state() -> Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let telemetry = Metrics::new()?;
        let state = Arc::new(ApiState::new(
            config.shared(),
            telemetry,
            Arc::new(build_openapi_document()),
            events.clone(),
            None,
        ));

        let Json(health_response) = health(State(state.clone())).await.expect("health");
        assert_eq!(health_response.status, "ok");

        let Json(full) = health_full(State(state.clone()))
            .await
            .expect("health full");
        assert_eq!(full.status, "ok");

        let response = super::metrics(State(state.clone())).await.expect("metrics");
        assert_eq!(response.status(), StatusCode::OK);

        config.set_fail_snapshot(true);
        let degraded = health(State(state.clone())).await;
        assert!(degraded.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn sse_stream_resumes_from_last_event() {
        let bus = EventBus::with_capacity(32);
        let torrent_id_1 = Uuid::new_v4();
        let torrent_id_2 = Uuid::new_v4();

        let first_id = bus.publish(CoreEvent::Completed {
            torrent_id: torrent_id_1,
            library_path: "/library/a".to_string(),
        });
        let second_id = bus.publish(CoreEvent::Completed {
            torrent_id: torrent_id_2,
            library_path: "/library/b".to_string(),
        });

        let stream = event_replay_stream(bus.clone(), Some(first_id));
        pin_mut!(stream);
        let envelope = stream.next().await.expect("expected event");

        assert_eq!(envelope.id, second_id);
        match envelope.event {
            CoreEvent::Completed { torrent_id, .. } => assert_eq!(torrent_id, torrent_id_2),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn rate_limiter_blocks_after_burst_exhausted() {
        let limit = ApiKeyRateLimit {
            burst: 2,
            replenish_period: Duration::from_secs(60),
        };
        let mut limiter = RateLimiter::new(limit.clone());
        let start = Instant::now();
        assert!(limiter.allow(&limit, start));
        assert!(limiter.allow(&limit, start));
        assert!(!limiter.allow(&limit, start + Duration::from_secs(1)));
    }

    #[test]
    fn rate_limiter_refills_after_period() {
        let limit = ApiKeyRateLimit {
            burst: 1,
            replenish_period: Duration::from_secs(1),
        };
        let mut limiter = RateLimiter::new(limit.clone());
        let start = Instant::now();
        assert!(limiter.allow(&limit, start));
        assert!(!limiter.allow(&limit, start + Duration::from_millis(100)));
        let later = start + Duration::from_secs(2);
        assert!(limiter.allow(&limit, later));
    }

    #[tokio::test]
    async fn sse_stream_emits_event_for_torrent_added() {
        let bus = EventBus::with_capacity(16);
        let publisher = bus.clone();
        let torrent_id = Uuid::new_v4();
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id,
                name: "example".to_string(),
            });
        });
        let stream = event_sse_stream(bus.clone(), None, SseFilter::default());
        pin_mut!(stream);
        match timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for SSE event")
        {
            Some(Ok(_)) => {}
            other => panic!("expected SSE event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn sse_filter_by_torrent_id() {
        let bus = EventBus::with_capacity(16);
        let target = Uuid::new_v4();
        let other = Uuid::new_v4();
        let publisher = bus.clone();

        tokio::spawn(async move {
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id: other,
                name: "other".to_string(),
            });
            let _ = publisher.publish(CoreEvent::TorrentAdded {
                torrent_id: target,
                name: "matching".to_string(),
            });
        });

        let mut filter = SseFilter::default();
        filter.torrent_ids.insert(target);

        let stream = event_replay_stream(bus, None).filter(move |envelope| {
            let filter = filter.clone();
            future::ready(matches_sse_filter(envelope, &filter))
        });
        pin_mut!(stream);
        let envelope = timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for filtered event")
            .expect("stream terminated");
        match envelope.event {
            CoreEvent::TorrentAdded { torrent_id, .. } => assert_eq!(torrent_id, target),
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn map_config_error_exposes_pointer_for_immutable_field() {
        let err = ConfigError::ImmutableField {
            section: "app_profile".to_string(),
            field: "instance_name".to_string(),
        };
        let api_error = map_config_error(err.into(), "failed");
        assert_eq!(api_error.status, StatusCode::UNPROCESSABLE_ENTITY);
        let params = api_error
            .invalid_params
            .expect("immutable field should set invalid params");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].pointer, "/app_profile/instance_name");
        assert!(
            params[0].message.contains("immutable"),
            "message should mention immutability"
        );
    }

    #[test]
    fn map_config_error_handles_root_pointer() {
        let err = ConfigError::InvalidField {
            section: "engine_profile".to_string(),
            field: "<root>".to_string(),
            message: "changeset must be a JSON object".to_string(),
        };
        let api_error = map_config_error(err.into(), "failed");
        let params = api_error
            .invalid_params
            .expect("invalid field should set invalid params");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].pointer, "/engine_profile");
        assert!(
            params[0].message.contains("must be a JSON object"),
            "message should echo validation failure"
        );
    }

    #[test]
    fn torrent_status_response_formats_state() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let status = TorrentStatus {
            id,
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Failed {
                message: "disk quota exceeded".to_string(),
            },
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1024,
                eta_seconds: Some(90),
            },
            rates: TorrentRates {
                download_bps: 2_048,
                upload_bps: 512,
                ratio: 0.5,
            },
            files: None,
            library_path: None,
            download_dir: None,
            sequential: false,
            added_at: now,
            completed_at: None,
            last_updated: now,
        };

        let detail = detail_from_components(status, TorrentMetadata::default());
        assert_eq!(detail.summary.id, id);
        assert_eq!(detail.summary.state.kind, TorrentStateKind::Failed);
        assert_eq!(
            detail.summary.state.failure_message.as_deref(),
            Some("disk quota exceeded")
        );
        assert_eq!(detail.summary.progress.bytes_downloaded, 512);
        assert_eq!(detail.summary.progress.bytes_total, 1024);
        assert!((detail.summary.progress.percent_complete - 50.0).abs() < f64::EPSILON);
        assert_eq!(detail.summary.progress.eta_seconds, Some(90));
        assert_eq!(detail.summary.rates.download_bps, 2_048);
        assert_eq!(detail.summary.rates.upload_bps, 512);
        assert!((detail.summary.rates.ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(detail.summary.added_at, now);
        assert!(detail.summary.completed_at.is_none());
    }

    #[tokio::test]
    async fn sse_stream_waits_for_new_events_after_reconnect() {
        let bus = EventBus::with_capacity(32);
        let torrent_id = Uuid::new_v4();
        let last_id = bus.publish(CoreEvent::Completed {
            torrent_id,
            library_path: "/library/a".to_string(),
        });

        let stream = event_replay_stream(bus.clone(), Some(last_id));
        pin_mut!(stream);

        let (tx, rx) = oneshot::channel();
        let publisher = bus.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let next = publisher.publish(CoreEvent::Completed {
                torrent_id: Uuid::new_v4(),
                library_path: "/library/b".to_string(),
            });
            let _ = tx.send(next);
        });

        let envelope = stream.next().await.expect("expected event");
        let next_id = rx.await.expect("publish id");
        assert_eq!(envelope.id, next_id);
    }

    #[derive(Default)]
    struct StubInspector {
        statuses: Mutex<Vec<TorrentStatus>>,
    }

    impl StubInspector {
        fn with_statuses(statuses: Vec<TorrentStatus>) -> Self {
            Self {
                statuses: Mutex::new(statuses),
            }
        }
    }

    #[async_trait]
    impl TorrentInspector for StubInspector {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            let snapshot = self.statuses.lock().await.clone();
            Ok(snapshot)
        }

        async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            let snapshot = self.statuses.lock().await.clone();
            Ok(snapshot.into_iter().find(|status| status.id == id))
        }
    }

    #[tokio::test]
    async fn fetch_all_torrents_returns_statuses() {
        let workflow = Arc::new(RecordingWorkflow::default());
        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let now = Utc::now();
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1_024,
                eta_seconds: Some(120),
            },
            rates: TorrentRates {
                download_bps: 4_096,
                upload_bps: 1_024,
                ratio: 0.5,
            },
            files: None,
            library_path: None,
            download_dir: Some("/downloads".to_string()),
            sequential: true,
            added_at: now,
            completed_at: None,
            last_updated: now,
        };
        let inspector = Arc::new(StubInspector::with_statuses(vec![sample_status.clone()]));
        let inspector_trait: Arc<dyn TorrentInspector> = inspector.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        let statuses = fetch_all_torrents(&handles)
            .await
            .expect("torrent statuses");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].state, TorrentState::Downloading);
        assert_eq!(statuses[0].name.as_deref(), Some("ubuntu.iso"));
    }

    #[tokio::test]
    async fn fetch_torrent_status_respects_not_found() {
        let workflow = Arc::new(RecordingWorkflow::default());
        let inspector = Arc::new(StubInspector::default());
        let handles = TorrentHandles::new(
            workflow.clone() as Arc<dyn TorrentWorkflow>,
            inspector.clone() as Arc<dyn TorrentInspector>,
        );
        let result = fetch_torrent_status(&handles, Uuid::new_v4()).await;
        match result {
            Err(err) => assert_eq!(err.status, StatusCode::NOT_FOUND),
            Ok(_) => panic!("expected torrent lookup to fail"),
        }
    }

    #[derive(Default)]
    struct RecordingWorkflow {
        added: Mutex<Vec<AddTorrent>>,
        removed: Mutex<Vec<(Uuid, RemoveTorrent)>>,
        should_fail_add: bool,
        should_fail_remove: bool,
    }

    #[async_trait]
    impl TorrentWorkflow for RecordingWorkflow {
        async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()> {
            if self.should_fail_add {
                anyhow::bail!("injected failure");
            }
            self.added.lock().await.push(request);
            Ok(())
        }

        async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()> {
            if self.should_fail_remove {
                anyhow::bail!("remove failure");
            }
            self.removed.lock().await.push((id, options));
            Ok(())
        }
    }

    #[async_trait]
    impl TorrentInspector for RecordingWorkflow {
        async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>> {
            Ok(Vec::new())
        }

        async fn get(&self, _id: Uuid) -> anyhow::Result<Option<TorrentStatus>> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn create_torrent_requires_workflow() {
        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some("magnet:?xt=urn:btih:example".to_string()),
            name: Some("example".to_string()),
            ..TorrentCreateRequest::default()
        };

        let err = dispatch_torrent_add(None, &request)
            .await
            .expect_err("expected workflow to be unavailable");
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn create_torrent_invokes_workflow() -> anyhow::Result<()> {
        let workflow = Arc::new(RecordingWorkflow::default());
        let request = TorrentCreateRequest {
            id: Uuid::new_v4(),
            magnet: Some("magnet:?xt=urn:btih:ubuntu".to_string()),
            name: Some("ubuntu.iso".to_string()),
            sequential: Some(true),
            include: vec!["*/include.mkv".to_string()],
            skip_fluff: true,
            max_download_bps: Some(1_000_000),
            ..TorrentCreateRequest::default()
        };

        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let inspector_trait: Arc<dyn TorrentInspector> = workflow.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        dispatch_torrent_add(Some(&handles), &request)
            .await
            .expect("torrent creation should succeed");
        let recorded_entry = {
            let recorded = workflow.added.lock().await;
            assert_eq!(recorded.len(), 1);
            recorded[0].clone()
        };
        assert_eq!(recorded_entry.id, request.id);
        match &recorded_entry.source {
            TorrentSource::Magnet { uri } => {
                assert!(uri.contains("ubuntu"));
            }
            TorrentSource::Metainfo { .. } => panic!("expected magnet source"),
        }
        assert_eq!(
            recorded_entry.options.name_hint.as_deref(),
            request.name.as_deref()
        );
        assert_eq!(recorded_entry.options.sequential, Some(true));
        assert_eq!(recorded_entry.options.file_rules.include, request.include);
        assert!(recorded_entry.options.file_rules.skip_fluff);
        assert_eq!(
            recorded_entry.options.rate_limit.download_bps,
            request.max_download_bps
        );
        Ok(())
    }

    #[test]
    fn summary_includes_metadata() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let status = TorrentStatus {
            id,
            name: Some("demo".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 42,
                bytes_total: 42,
                eta_seconds: None,
            },
            rates: TorrentRates::default(),
            files: None,
            library_path: Some("/library/demo".to_string()),
            download_dir: None,
            sequential: false,
            added_at: now,
            completed_at: Some(now),
            last_updated: now,
        };
        let metadata = TorrentMetadata::new(
            vec!["tagA".to_string(), "tagB".to_string()],
            vec!["http://tracker".to_string()],
        );
        let summary = summary_from_components(status, metadata);
        assert_eq!(summary.tags, vec!["tagA".to_string(), "tagB".to_string()]);
        assert_eq!(summary.trackers, vec!["http://tracker".to_string()]);
    }

    #[tokio::test]
    async fn delete_torrent_requires_workflow() {
        let id = Uuid::new_v4();
        let err = dispatch_torrent_remove(None, id)
            .await
            .expect_err("expected workflow to be unavailable");
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn delete_torrent_invokes_workflow() -> anyhow::Result<()> {
        let workflow = Arc::new(RecordingWorkflow::default());
        let id = Uuid::new_v4();

        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let inspector_trait: Arc<dyn TorrentInspector> = workflow.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        dispatch_torrent_remove(Some(&handles), id)
            .await
            .expect("torrent removal should succeed");

        {
            let recorded = workflow.removed.lock().await;
            assert_eq!(recorded.len(), 1);
            assert_eq!(recorded[0].0, id);
            drop(recorded);
        }
        Ok(())
    }

    #[test]
    fn decode_cursor_token_rejects_invalid_base64() {
        let err = decode_cursor_token("%%%");
        assert!(err.is_err(), "invalid cursor token should error");
        let api_err = err.expect_err("expected error");
        assert_eq!(api_err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn cursor_token_round_trip_preserves_identity() {
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            last_updated: Utc::now(),
            ..TorrentStatus::default()
        };
        let entry = StatusEntry {
            status: status.clone(),
            metadata: TorrentMetadata::new(vec![], vec![]),
        };

        let encoded = encode_cursor_from_entry(&entry).expect("cursor encoding should succeed");
        let decoded = decode_cursor_token(&encoded).expect("cursor decoding should succeed");
        assert_eq!(decoded.id, status.id);
        assert_eq!(decoded.last_updated, status.last_updated);
    }

    #[test]
    fn parse_state_filter_rejects_unknown_value() {
        let err =
            parse_state_filter("mystery").expect_err("unexpected success for unknown state filter");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn comma_splitter_trims_and_lowercases() {
        let values = split_comma_separated(" Alpha , ,BETA ,gamma ");
        assert_eq!(values, vec!["alpha", "beta", "gamma"]);
    }

    #[cfg(feature = "compat-qb")]
    const QB_TEST_MAGNET: &str = "magnet:?xt=urn:btih:revaerqb";

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_sync_maindata_maps_status() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("sample".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 256,
                bytes_total: 512,
                eta_seconds: Some(30),
            },
            rates: TorrentRates {
                download_bps: 1_024,
                upload_bps: 256,
                ratio: 0.5,
            },
            download_dir: Some("/downloads".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let Json(response) = compat_qb::sync_maindata(State(state), Query(SyncParams::default()))
            .await
            .expect("sync");

        assert!(
            response
                .torrents
                .contains_key(&sample_status.id.simple().to_string())
        );
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrents_add_records_submission() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let form = TorrentAddForm {
            urls: Some(QB_TEST_MAGNET.to_string()),
            tags: Some("alpha,beta".to_string()),
            ..TorrentAddForm::default()
        };

        compat_qb::torrents_add(State(state), Form(form))
            .await
            .expect("add torrent");

        assert_eq!(stub.added().await.len(), 1);
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[test]
    fn qb_parse_limit_handles_unlimited() {
        assert_eq!(compat_qb::parse_limit("0").unwrap(), None);
        assert_eq!(compat_qb::parse_limit("-1").unwrap(), None);
        assert_eq!(compat_qb::parse_limit("1024").unwrap(), Some(1_024));
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrents_info_filters_hashes() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("sample".to_string()),
            state: TorrentState::Seeding,
            progress: TorrentProgress {
                bytes_downloaded: 1_024,
                bytes_total: 1_024,
                eta_seconds: Some(0),
            },
            rates: TorrentRates {
                download_bps: 0,
                upload_bps: 512,
                ratio: 1.0,
            },
            download_dir: Some("/downloads/sample".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let params = TorrentsInfoParams {
            hashes: Some(sample_status.id.simple().to_string()),
        };

        let Json(entries) = compat_qb::torrents_info(State(state), Query(params))
            .await
            .expect("torrents info");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hash, sample_status.id.simple().to_string());
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_torrent_pause_resume_apply_actions() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("pause-me".to_string()),
            state: TorrentState::Downloading,
            download_dir: Some("/downloads".to_string()),
            sequential: false,
            ..TorrentStatus::default()
        };
        stub.push_status(sample_status.clone()).await;
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let hashes = sample_status.id.simple().to_string();
        let form = TorrentHashesForm { hashes };

        compat_qb::torrents_pause(State(state.clone()), Form(form.clone()))
            .await
            .expect("pause");
        compat_qb::torrents_resume(State(state), Form(form))
            .await
            .expect("resume");

        let actions = stub.actions().await;
        assert!(actions.iter().any(|(_, action)| action == "pause"));
        assert!(actions.iter().any(|(_, action)| action == "resume"));
        Ok(())
    }

    #[cfg(feature = "compat-qb")]
    #[tokio::test]
    async fn qb_transfer_limits_accept_positive_values() -> anyhow::Result<()> {
        let config = MockConfig::new();
        config.set_app_mode(AppMode::Active).await;
        let events = EventBus::with_capacity(8);
        let metrics = Metrics::new()?;
        let stub = Arc::new(StubTorrent::default());
        let handles = TorrentHandles::new(stub.clone(), stub.clone());
        let state = Arc::new(ApiState::new(
            config.shared(),
            metrics,
            Arc::new(build_openapi_document()),
            events,
            Some(handles),
        ));

        let form = TransferLimitForm {
            limit: "2048".to_string(),
        };
        compat_qb::transfer_upload_limit(State(state.clone()), Form(form.clone()))
            .await
            .expect("upload limit");
        compat_qb::transfer_download_limit(State(state), Form(form))
            .await
            .expect("download limit");
        Ok(())
    }

    #[test]
    fn normalise_lower_trims_whitespace() {
        assert_eq!(normalise_lower("  HeLLo "), "hello");
    }

    #[test]
    fn build_sse_filter_rejects_unknown_event_kind() {
        let query = SseQuery {
            event: Some("not_a_real_event".to_string()),
            ..SseQuery::default()
        };
        let err = build_sse_filter(&query).expect_err("unknown event kind should be rejected");
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn build_sse_filter_parses_filters() {
        let torrent_id = Uuid::new_v4();
        let query = SseQuery {
            torrent: Some(format!("{torrent_id}")),
            event: Some("progress,completed".to_string()),
            state: Some("Completed".to_string()),
        };

        let filter = build_sse_filter(&query).expect("filter construction should succeed");
        assert!(filter.torrent_ids.contains(&torrent_id));
        assert!(filter.event_kinds.contains("progress"));
        assert!(filter.event_kinds.contains("completed"));
        assert!(filter.states.contains(&TorrentStateKind::Completed));
    }

    #[test]
    fn matches_sse_filter_respects_state_and_ids() {
        let torrent_id = Uuid::new_v4();
        let mut filter = SseFilter::default();
        filter.torrent_ids.insert(torrent_id);
        filter.states.insert(TorrentStateKind::Completed);

        let completed = EventEnvelope {
            id: 1,
            timestamp: Utc::now(),
            event: CoreEvent::Completed {
                torrent_id,
                library_path: "/library/demo".to_string(),
            },
        };
        assert!(matches_sse_filter(&completed, &filter));

        let wrong_id = EventEnvelope {
            id: 2,
            timestamp: Utc::now(),
            event: CoreEvent::Completed {
                torrent_id: Uuid::new_v4(),
                library_path: "/library/other".to_string(),
            },
        };
        assert!(!matches_sse_filter(&wrong_id, &filter));

        let wrong_state = EventEnvelope {
            id: 3,
            timestamp: Utc::now(),
            event: CoreEvent::StateChanged {
                torrent_id,
                state: TorrentState::Downloading,
            },
        };
        assert!(!matches_sse_filter(&wrong_state, &filter));
    }

    #[test]
    fn detail_from_components_embeds_metadata() {
        let now = Utc::now();
        let status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("demo".to_string()),
            state: TorrentState::Completed,
            progress: TorrentProgress {
                bytes_downloaded: 100,
                bytes_total: 100,
                eta_seconds: None,
            },
            rates: TorrentRates::default(),
            files: None,
            library_path: Some("/library/demo".to_string()),
            download_dir: None,
            sequential: false,
            added_at: now,
            completed_at: Some(now),
            last_updated: now,
        };
        let metadata =
            TorrentMetadata::new(vec!["tag".to_string()], vec!["http://tracker".to_string()]);

        let detail = detail_from_components(status, metadata);
        assert_eq!(detail.summary.tags, vec!["tag".to_string()]);
        assert_eq!(detail.summary.trackers, vec!["http://tracker".to_string()]);
    }
}
