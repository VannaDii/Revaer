use std::convert::{Infallible, TryFrom};
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::Result;
use async_stream::stream;
use axum::{
    body::Body,
    extract::{Extension, MatchedPath, Path as AxumPath, State},
    http::{header::CONTENT_TYPE, HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{self, Sse},
        IntoResponse, Response,
    },
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use revaer_config::{
    AppMode, ConfigError, ConfigService, ConfigSnapshot, SettingsChangeset, SettingsFacade,
};
use revaer_events::{
    DiscoveredFile, Event as CoreEvent, EventBus, EventEnvelope, EventId, TorrentState,
};
use revaer_telemetry::{
    build_sha, record_app_mode, set_request_context, with_request_context, Metrics,
};
use revaer_torrent_core::{TorrentDescriptor, TorrentInspector, TorrentStatus, TorrentWorkflow};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::net::TcpListener;
use tower::{layer::Layer, Service, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn, Span};
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

struct ApiState {
    config: ConfigService,
    setup_token_ttl: Duration,
    telemetry: Metrics,
    openapi_document: Arc<Value>,
    events: EventBus,
    health_status: Mutex<Option<Vec<String>>>,
    torrent: Option<TorrentHandles>,
}

impl ApiState {
    fn new(
        config: ConfigService,
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
            health_status: Mutex::new(None),
            torrent,
        }
    }

    fn record_health_status(&self, degraded: Vec<String>) {
        let mut last = self
            .health_status
            .lock()
            .expect("health status mutex poisoned");
        if last.as_ref() != Some(&degraded) {
            *last = Some(degraded.clone());
            let _ = self.events.publish(CoreEvent::HealthChanged { degraded });
        }
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
}

#[derive(Clone)]
enum AuthContext {
    SetupToken(String),
    ApiKey { key_id: String },
}

#[derive(Clone)]
struct HttpMetricsLayer {
    telemetry: Metrics,
}

impl HttpMetricsLayer {
    fn new(telemetry: Metrics) -> Self {
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

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
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

#[derive(Serialize)]
struct ProblemDetails {
    #[serde(rename = "type")]
    kind: String,
    title: String,
    status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    kind: &'static str,
    title: &'static str,
    detail: Option<String>,
}

impl ApiError {
    fn new(status: StatusCode, kind: &'static str, title: &'static str) -> Self {
        Self {
            status,
            kind,
            title,
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
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
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ProblemDetails {
            kind: self.kind.to_string(),
            title: self.title.to_string(),
            status: self.status.as_u16(),
            detail: self.detail,
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
struct TorrentProgressResponse {
    bytes_downloaded: u64,
    bytes_total: u64,
    percent_complete: f64,
}

#[derive(Serialize)]
struct TorrentStatusResponse {
    id: Uuid,
    name: Option<String>,
    state: String,
    failure_message: Option<String>,
    progress: TorrentProgressResponse,
    files: Option<Vec<DiscoveredFile>>,
    library_path: Option<String>,
    last_updated: DateTime<Utc>,
}

impl From<TorrentStatus> for TorrentStatusResponse {
    fn from(status: TorrentStatus) -> Self {
        let (state, failure_message) = match status.state {
            TorrentState::Queued => ("queued".to_string(), None),
            TorrentState::FetchingMetadata => ("fetching_metadata".to_string(), None),
            TorrentState::Downloading => ("downloading".to_string(), None),
            TorrentState::Seeding => ("seeding".to_string(), None),
            TorrentState::Completed => ("completed".to_string(), None),
            TorrentState::Failed { message } => ("failed".to_string(), Some(message)),
            TorrentState::Stopped => ("stopped".to_string(), None),
        };

        Self {
            id: status.id,
            name: status.name,
            state,
            failure_message,
            progress: TorrentProgressResponse {
                bytes_downloaded: status.progress.bytes_downloaded,
                bytes_total: status.progress.bytes_total,
                percent_complete: status.progress.percent_complete(),
            },
            files: status.files,
            library_path: status.library_path,
            last_updated: status.last_updated,
        }
    }
}

#[derive(Clone, Deserialize)]
struct TorrentCreateRequest {
    id: Uuid,
    name: String,
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
    ) -> Result<Self> {
        let telemetry = Metrics::new()?;
        let openapi_document = Arc::new(build_openapi_document());
        revaer_telemetry::persist_openapi(Path::new("docs/api/openapi.json"), &openapi_document)?;

        let state = Arc::new(ApiState::new(
            config,
            telemetry.clone(),
            openapi_document.clone(),
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
            .layer(HttpMetricsLayer::new(telemetry.clone()));

        let router = Router::new()
            .route("/health", get(health))
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
            .route("/v1/events", get(stream_events))
            .route("/metrics", get(metrics))
            .route("/docs/openapi.json", get(openapi_document_handler))
            .route_layer(layered)
            .with_state(state);

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
    let token = match context {
        AuthContext::SetupToken(token) => token,
        AuthContext::ApiKey { .. } => {
            return Err(ApiError::internal(
                "invalid authentication context for setup completion",
            ))
        }
    };

    if let Err(err) = state.config.validate_setup_token(&token).await {
        warn!(error = %err, "setup token validation failed");
        return Err(ApiError::unauthorized("invalid setup token"));
    }

    let updated_app_patch = match changeset.app_profile.take() {
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
    changeset.app_profile = Some(updated_app_patch);

    state
        .config
        .apply_changeset("setup", "setup_complete", changeset)
        .await
        .map_err(|err| map_config_error(err, "failed to apply setup changes"))?;

    if let Err(err) = state.config.consume_setup_token(&token).await {
        error!(error = %err, "failed to consume setup token after completion");
        return Err(ApiError::internal("failed to finalize setup"));
    }

    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;

    let _ = state.events.publish(CoreEvent::SettingsChanged {
        description: format!("setup_complete revision {}", snapshot.revision),
    });

    Ok(Json(snapshot))
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
            ))
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
            ))
        }
    }

    dispatch_torrent_add(state.torrent.as_ref(), &request).await?;
    info!(torrent_id = %request.id, torrent_name = %request.name, "torrent submission requested");
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
            ))
        }
    }

    dispatch_torrent_remove(state.torrent.as_ref(), id).await?;
    info!(torrent_id = %id, "torrent removal requested");
    state.update_torrent_metrics().await;
    Ok(StatusCode::NO_CONTENT)
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

async fn list_torrents(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Vec<TorrentStatusResponse>>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let statuses = fetch_all_torrents(handles).await?;
    state.record_torrent_metrics(&statuses);
    Ok(Json(
        statuses
            .into_iter()
            .map(TorrentStatusResponse::from)
            .collect(),
    ))
}

async fn get_torrent(
    State(state): State<Arc<ApiState>>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<TorrentStatusResponse>, ApiError> {
    let handles = state
        .torrent
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;
    let status = fetch_torrent_status(handles, id).await?;
    state.record_torrent_metrics(std::slice::from_ref(&status));
    Ok(Json(status.into()))
}

async fn health(State(state): State<Arc<ApiState>>) -> Result<Json<HealthResponse>, ApiError> {
    match state.config.snapshot().await {
        Ok(snapshot) => {
            state.record_health_status(Vec::new());
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
            state.record_health_status(vec!["database".to_string()]);
            warn!(error = %err, "health check failed to reach database");
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
) -> Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send> {
    let last_id = headers
        .get(HEADER_LAST_EVENT_ID)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<EventId>().ok());

    let stream = event_sse_stream(&state.events, last_id);

    Sse::new(stream).keep_alive(
        sse::KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
            .text("keep-alive"),
    )
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
    bus: &EventBus,
    since: Option<EventId>,
) -> impl futures_core::Stream<Item = EventEnvelope> + Send {
    let mut stream = bus.subscribe(since);
    stream! {
        while let Some(envelope) = stream.next().await {
            yield envelope;
        }
    }
}

fn event_sse_stream(
    bus: &EventBus,
    since: Option<EventId>,
) -> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    event_replay_stream(bus, since).filter_map(|envelope| async move {
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
            "/admin/torrents": {
                "get": {
                    "summary": "List torrents and their current status",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "responses": {
                        "200": {
                            "description": "Current torrent catalogue",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/TorrentStatusResponse" }
                                    }
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
                        "401": {
                            "description": "Authentication failed",
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
            "/admin/torrents/{id}": {
                "get": {
                    "summary": "Fetch torrent status by identifier",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Torrent status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/TorrentStatusResponse" }
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
                },
                "delete": {
                    "summary": "Request torrent removal",
                    "security": [ { "ApiKeyAuth": [] } ],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "204": {
                            "description": "Torrent removal requested"
                        },
                        "401": {
                            "description": "Authentication failed",
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
                        "name": { "type": "string" }
                    },
                    "required": ["id", "name"]
                },
                "DiscoveredFile": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "size_bytes": { "type": "integer", "format": "int64" }
                    },
                    "required": ["path", "size_bytes"]
                },
                "TorrentProgressResponse": {
                    "type": "object",
                    "properties": {
                        "bytes_downloaded": { "type": "integer", "format": "int64" },
                        "bytes_total": { "type": "integer", "format": "int64" },
                        "percent_complete": { "type": "number", "format": "float" }
                    },
                    "required": ["bytes_downloaded", "bytes_total", "percent_complete"]
                },
                "TorrentStatusResponse": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "name": { "type": ["string", "null"] },
                        "state": { "type": "string" },
                        "failure_message": { "type": ["string", "null"] },
                        "progress": { "$ref": "#/components/schemas/TorrentProgressResponse" },
                        "files": {
                            "type": ["array", "null"],
                            "items": { "$ref": "#/components/schemas/DiscoveredFile" }
                        },
                        "library_path": { "type": ["string", "null"] },
                        "last_updated": { "type": "string", "format": "date-time" }
                    },
                    "required": ["id", "state", "progress", "last_updated"]
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

    let valid = state
        .config
        .verify_api_key(key_id, secret)
        .await
        .map_err(|err| {
            error!(error = %err, "failed to verify API key");
            ApiError::internal("failed to verify API key")
        })?;

    if !valid {
        return Err(ApiError::unauthorized("invalid API key"));
    }

    req.extensions_mut().insert(AuthContext::ApiKey {
        key_id: key_id.to_string(),
    });

    Ok(next.run(req).await)
}

fn map_config_error(err: anyhow::Error, context: &'static str) -> ApiError {
    match err.downcast::<ConfigError>() {
        Ok(config_err) => {
            warn!(error = %config_err, "{context}");
            ApiError::config_invalid(config_err.to_string())
        }
        Err(other) => {
            error!(error = %other, "{context}");
            ApiError::internal(context)
        }
    }
}

async fn dispatch_torrent_add(
    handles: Option<&TorrentHandles>,
    request: &TorrentCreateRequest,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    handles
        .workflow()
        .add_torrent(TorrentDescriptor {
            id: request.id,
            name: request.name.clone(),
        })
        .await
        .map_err(|err| {
            error!(error = %err, "failed to add torrent through workflow");
            ApiError::internal("failed to add torrent")
        })
}

async fn dispatch_torrent_remove(
    handles: Option<&TorrentHandles>,
    id: Uuid,
) -> Result<(), ApiError> {
    let handles =
        handles.ok_or_else(|| ApiError::service_unavailable("torrent workflow not configured"))?;

    handles.workflow().remove_torrent(id).await.map_err(|err| {
        error!(error = %err, "failed to remove torrent through workflow");
        ApiError::internal("failed to remove torrent")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::http::StatusCode;
    use chrono::Utc;
    use futures_util::{pin_mut, StreamExt};
    use revaer_torrent_core::{
        TorrentDescriptor, TorrentInspector, TorrentProgress, TorrentStatus, TorrentWorkflow,
    };
    use std::sync::Arc;
    use tokio::sync::{oneshot, Mutex};
    use tokio::time::{sleep, timeout, Duration};
    use uuid::Uuid;

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

        let stream = event_replay_stream(&bus, Some(first_id));
        pin_mut!(stream);
        let envelope = stream.next().await.expect("expected event");

        assert_eq!(envelope.id, second_id);
        match envelope.event {
            CoreEvent::Completed { torrent_id, .. } => assert_eq!(torrent_id, torrent_id_2),
            other => panic!("unexpected event {other:?}"),
        }
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
        let stream = event_sse_stream(&bus, None);
        pin_mut!(stream);
        match timeout(Duration::from_millis(200), stream.next())
            .await
            .expect("timed out waiting for SSE event")
        {
            Some(Ok(_)) => {}
            other => panic!("expected SSE event, got {other:?}"),
        }
    }

    #[test]
    fn torrent_status_response_formats_state() {
        let id = Uuid::new_v4();
        let status = TorrentStatus {
            id,
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Failed {
                message: "disk quota exceeded".to_string(),
            },
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1024,
            },
            files: None,
            library_path: None,
            last_updated: Utc::now(),
        };

        let response: TorrentStatusResponse = status.into();
        assert_eq!(response.id, id);
        assert_eq!(response.state, "failed");
        assert_eq!(
            response.failure_message.as_deref(),
            Some("disk quota exceeded")
        );
        assert_eq!(response.progress.bytes_downloaded, 512);
        assert_eq!(response.progress.bytes_total, 1024);
        assert!((response.progress.percent_complete - 50.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn sse_stream_waits_for_new_events_after_reconnect() {
        let bus = EventBus::with_capacity(32);
        let torrent_id = Uuid::new_v4();
        let last_id = bus.publish(CoreEvent::Completed {
            torrent_id,
            library_path: "/library/a".to_string(),
        });

        let stream = event_replay_stream(&bus, Some(last_id));
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
        let sample_status = TorrentStatus {
            id: Uuid::new_v4(),
            name: Some("ubuntu.iso".to_string()),
            state: TorrentState::Downloading,
            progress: TorrentProgress {
                bytes_downloaded: 512,
                bytes_total: 1024,
            },
            files: None,
            library_path: None,
            last_updated: Utc::now(),
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
        added: Mutex<Vec<TorrentDescriptor>>,
        removed: Mutex<Vec<Uuid>>,
        should_fail_add: bool,
        should_fail_remove: bool,
    }

    #[async_trait]
    impl TorrentWorkflow for RecordingWorkflow {
        async fn add_torrent(&self, descriptor: TorrentDescriptor) -> anyhow::Result<()> {
            if self.should_fail_add {
                anyhow::bail!("injected failure");
            }
            let mut guard = self.added.lock().await;
            guard.push(descriptor);
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid) -> anyhow::Result<()> {
            if self.should_fail_remove {
                anyhow::bail!("remove failure");
            }
            let mut guard = self.removed.lock().await;
            guard.push(_id);
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
            name: "example".to_string(),
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
            name: "ubuntu.iso".to_string(),
        };

        let workflow_trait: Arc<dyn TorrentWorkflow> = workflow.clone();
        let inspector_trait: Arc<dyn TorrentInspector> = workflow.clone();
        let handles = TorrentHandles::new(workflow_trait, inspector_trait);

        dispatch_torrent_add(Some(&handles), &request)
            .await
            .expect("torrent creation should succeed");
        let recorded = workflow.added.lock().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].id, request.id);
        assert_eq!(recorded[0].name, request.name);
        Ok(())
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

        let recorded = workflow.removed.lock().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0], id);
        Ok(())
    }
}
