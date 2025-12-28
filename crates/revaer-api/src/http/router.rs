//! Router construction and server host for the API.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::{
    Router,
    http::{HeaderName, Method, Request, header::CONTENT_TYPE},
    middleware,
    routing::{get, patch, post, put},
};
use revaer_config::ConfigService;
use revaer_events::EventBus;
use revaer_telemetry::{Metrics, build_sha};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;

use crate::TorrentHandles;
use crate::app::state::ApiState;
use crate::config::SharedConfig;
use crate::http::auth::{require_api_key, require_factory_reset_auth, require_setup_token};
#[cfg(feature = "compat-qb")]
use crate::http::compat_qb;
use crate::http::constants::{
    HEADER_API_KEY, HEADER_API_KEY_LEGACY, HEADER_LAST_EVENT_ID, HEADER_REQUEST_ID,
    HEADER_SETUP_TOKEN,
};
use crate::http::health::{dashboard, health, health_full, metrics};
use crate::http::settings::{factory_reset, get_config_snapshot, settings_patch, well_known};
use crate::http::setup::{setup_complete, setup_start};
use crate::http::sse::stream_events;
use crate::http::telemetry::HttpMetricsLayer;
use crate::http::torrents::handlers::{
    action_torrent, create_torrent, create_torrent_authoring, delete_torrent, get_torrent,
    list_torrent_categories, list_torrent_peers, list_torrent_tags, list_torrent_trackers,
    list_torrents, remove_torrent_trackers, select_torrent, update_torrent_options,
    update_torrent_trackers, update_torrent_web_seeds, upsert_torrent_category, upsert_torrent_tag,
};
use crate::openapi::OpenApiDependencies;

/// Axum router wrapper that hosts the Revaer API services.
pub struct ApiServer {
    router: Router,
}

impl ApiServer {
    /// Construct a new API server with shared dependencies wired through application state.
    ///
    /// # Errors
    ///
    /// Returns an error if telemetry cannot be initialized or if persisting the `OpenAPI` document
    /// fails.
    pub fn new(
        config: ConfigService,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
    ) -> Result<Self> {
        let openapi = OpenApiDependencies::embedded_at(Path::new("docs/api/openapi.json"));
        Self::with_config(Arc::new(config), events, torrent, telemetry, &openapi)
    }

    fn with_config(
        config: SharedConfig,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> Result<Self> {
        Self::with_config_at(config, events, torrent, telemetry, openapi)
    }

    pub(crate) fn with_config_at(
        config: SharedConfig,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> Result<Self> {
        Self::with_dependencies(config, events, torrent, telemetry, openapi)
    }

    fn with_dependencies(
        config: SharedConfig,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> Result<Self> {
        (openapi.persist)(&openapi.path, &openapi.document)?;
        let state = Self::build_state(
            config,
            telemetry.clone(),
            Arc::clone(&openapi.document),
            events,
            torrent,
        );
        let cors_layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                CONTENT_TYPE,
                HeaderName::from_static(HEADER_API_KEY),
                HeaderName::from_static(HEADER_API_KEY_LEGACY),
                HeaderName::from_static(HEADER_SETUP_TOKEN),
                HeaderName::from_static(HEADER_LAST_EVENT_ID),
            ]);
        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                let method = request.method().clone();
                let uri_path = request.uri().path();
                let request_id = request
                    .headers()
                    .get(HEADER_REQUEST_ID)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                let span = tracing::info_span!(
                    "http.request",
                    method = %method,
                    route = %uri_path,
                    request_id = %request_id,
                    mode = tracing::field::Empty,
                    build_sha = %build_sha(),
                    status_code = tracing::field::Empty,
                    latency_ms = tracing::field::Empty
                );
                span
            })
            .on_request(|_request: &Request<_>, _span: &Span| {})
            .on_response(
                |response: &axum::response::Response, latency: Duration, span: &Span| {
                    let status = response.status().as_u16();
                    span.record("status_code", status);
                    let latency_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX);
                    span.record("latency_ms", latency_ms);
                },
            );
        let layered = ServiceBuilder::new()
            .layer(revaer_telemetry::propagate_request_id_layer())
            .layer(revaer_telemetry::set_request_id_layer())
            .layer(trace_layer)
            .layer(HttpMetricsLayer::new(telemetry));

        let router = Self::build_router(&state);
        let router = Self::mount_optional_compat(router);
        let router = router
            .layer(cors_layer)
            .route_layer(layered)
            .with_state(state);

        Ok(Self { router })
    }

    pub(crate) fn build_state(
        config: SharedConfig,
        telemetry: Metrics,
        openapi_document: Arc<serde_json::Value>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
    ) -> Arc<ApiState> {
        Arc::new(ApiState::new(
            config,
            telemetry,
            openapi_document,
            events,
            torrent,
        ))
    }

    fn build_router(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        Self::public_routes()
            .merge(Self::admin_routes(state))
            .merge(Self::v1_routes(state))
    }

    fn public_routes() -> Router<Arc<ApiState>> {
        Router::new()
            .route("/health", get(health))
            .route("/health/full", get(health_full))
            .route("/.well-known/revaer.json", get(well_known))
            .route("/metrics", get(metrics))
            .route(
                "/docs/openapi.json",
                get(crate::http::docs::openapi_document_handler),
            )
    }

    fn admin_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_setup = middleware::from_fn_with_state(state.clone(), require_setup_token);
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);
        let require_factory_reset =
            middleware::from_fn_with_state(state.clone(), require_factory_reset_auth);

        Router::new()
            .route("/admin/setup/start", post(setup_start))
            .route(
                "/admin/setup/complete",
                post(setup_complete).route_layer(require_setup),
            )
            .route(
                "/admin/settings",
                patch(settings_patch).route_layer(require_api.clone()),
            )
            .route(
                "/admin/factory-reset",
                post(factory_reset).route_layer(require_factory_reset),
            )
            .route(
                "/admin/torrents",
                get(list_torrents)
                    .post(create_torrent)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/categories",
                get(list_torrent_categories).route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/categories/{name}",
                put(upsert_torrent_category).route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/tags",
                get(list_torrent_tags).route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/tags/{name}",
                put(upsert_torrent_tag).route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/create",
                post(create_torrent_authoring).route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/{id}",
                get(get_torrent)
                    .delete(delete_torrent)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/admin/torrents/{id}/peers",
                get(list_torrent_peers).route_layer(require_api),
            )
    }

    fn v1_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/dashboard",
                get(dashboard).route_layer(require_api.clone()),
            )
            .route(
                "/v1/config",
                get(get_config_snapshot)
                    .patch(settings_patch)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents",
                get(list_torrents)
                    .post(create_torrent)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/categories",
                get(list_torrent_categories).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/categories/{name}",
                put(upsert_torrent_category).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/tags",
                get(list_torrent_tags).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/tags/{name}",
                put(upsert_torrent_tag).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/create",
                post(create_torrent_authoring).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}",
                get(get_torrent).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/select",
                post(select_torrent).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/action",
                post(action_torrent).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/options",
                patch(update_torrent_options).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/trackers",
                get(list_torrent_trackers)
                    .patch(update_torrent_trackers)
                    .delete(remove_torrent_trackers)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/peers",
                get(list_torrent_peers).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/{id}/web_seeds",
                patch(update_torrent_web_seeds).route_layer(require_api.clone()),
            )
            .route(
                "/v1/events",
                get(stream_events).route_layer(require_api.clone()),
            )
            .route(
                "/v1/events/stream",
                get(stream_events).route_layer(require_api.clone()),
            )
            .route(
                "/v1/torrents/events",
                get(stream_events).route_layer(require_api),
            )
    }

    fn mount_optional_compat(router: Router<Arc<ApiState>>) -> Router<Arc<ApiState>> {
        #[cfg(feature = "compat-qb")]
        {
            compat_qb::mount(router)
        }
        #[cfg(not(feature = "compat-qb"))]
        {
            router
        }
    }

    /// Serve the API using the configured router on the supplied address.
    ///
    /// # Errors
    ///
    /// Returns an error if the listener fails to bind or the server terminates unexpectedly.
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        tracing::info!("Starting API on {}", addr);
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, self.router.into_make_service()).await?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn router(&self) -> &Router {
        &self.router
    }
}
