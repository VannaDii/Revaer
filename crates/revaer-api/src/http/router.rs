//! Router construction and server host for the API.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    http::{
        HeaderName, Method, Request,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware,
    routing::{delete, get, patch, post, put},
};
use revaer_config::ConfigService;
use revaer_events::EventBus;
use revaer_telemetry::{Metrics, build_sha};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::Span;

use crate::TorrentHandles;
use crate::app::indexers::IndexerFacade;
use crate::app::state::ApiState;
use crate::config::SharedConfig;
use crate::error::{ApiServerError, ApiServerResult};
use crate::http::auth::{require_api_key, require_factory_reset_auth, require_setup_token};
#[cfg(feature = "compat-qb")]
use crate::http::compat_qb;
use crate::http::constants::{
    HEADER_API_KEY, HEADER_API_KEY_LEGACY, HEADER_LAST_EVENT_ID, HEADER_REQUEST_ID,
    HEADER_SETUP_TOKEN,
};
use crate::http::filesystem::browse_filesystem;
use crate::http::health::{dashboard, health, health_full, metrics};
use crate::http::indexers as indexer_handlers;
use crate::http::logs::stream_logs;
use crate::http::settings::{factory_reset, get_config_snapshot, settings_patch, well_known};
use crate::http::setup::{setup_complete, setup_start};
use crate::http::sse::stream_events;
use crate::http::telemetry::HttpMetricsLayer;
use crate::http::tokens::refresh_api_key;
use crate::http::torrents::handlers::{
    action_torrent, create_torrent, create_torrent_authoring, delete_torrent, get_torrent,
    list_torrent_categories, list_torrent_peers, list_torrent_tags, list_torrent_trackers,
    list_torrents, remove_torrent_trackers, select_torrent, update_torrent_options,
    update_torrent_trackers, update_torrent_web_seeds, upsert_torrent_category, upsert_torrent_tag,
};
use crate::http::torznab::{torznab_api, torznab_download};
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
        indexers: Arc<dyn IndexerFacade>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
    ) -> ApiServerResult<Self> {
        let openapi_path = crate::openapi_output_path();
        let openapi = OpenApiDependencies::embedded_at(&openapi_path);
        Self::with_config(
            Arc::new(config),
            indexers,
            events,
            torrent,
            telemetry,
            &openapi,
        )
    }

    fn with_config(
        config: SharedConfig,
        indexers: Arc<dyn IndexerFacade>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> ApiServerResult<Self> {
        Self::with_config_at(config, indexers, events, torrent, telemetry, openapi)
    }

    pub(crate) fn with_config_at(
        config: SharedConfig,
        indexers: Arc<dyn IndexerFacade>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> ApiServerResult<Self> {
        Self::with_dependencies(config, indexers, events, torrent, telemetry, openapi)
    }

    fn with_dependencies(
        config: SharedConfig,
        indexers: Arc<dyn IndexerFacade>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
        telemetry: Metrics,
        openapi: &OpenApiDependencies,
    ) -> ApiServerResult<Self> {
        (openapi.persist)(&openapi.path, &openapi.document).map_err(|source| {
            ApiServerError::OpenApiPersist {
                path: openapi.path.clone(),
                source,
            }
        })?;
        let state = Self::build_state(
            config,
            indexers,
            telemetry.clone(),
            Arc::clone(&openapi.document),
            events,
            torrent,
        );
        let cors_layer = CorsLayer::new()
            .allow_origin(AllowOrigin::mirror_request())
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                AUTHORIZATION,
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
            .layer(middleware::from_fn(crate::i18n::with_locale))
            .layer(cors_layer)
            .route_layer(layered)
            .with_state(state);

        Ok(Self { router })
    }

    pub(crate) fn build_state(
        config: SharedConfig,
        indexers: Arc<dyn IndexerFacade>,
        telemetry: Metrics,
        openapi_document: Arc<serde_json::Value>,
        events: EventBus,
        torrent: Option<TorrentHandles>,
    ) -> Arc<ApiState> {
        Arc::new(ApiState::new(
            config,
            indexers,
            telemetry,
            openapi_document,
            events,
            torrent,
        ))
    }

    fn build_router(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        Self::public_routes()
            .merge(Self::admin_routes(state))
            .merge(Self::torznab_routes(state))
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

    fn torznab_routes(_state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        Router::new()
            .route(
                "/torznab/{torznab_instance_public_id}/api",
                get(torznab_api),
            )
            .route(
                "/torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}",
                get(torznab_download),
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
        Self::v1_core_routes(state)
            .merge(Self::v1_indexer_routes(state))
            .merge(Self::v1_torrent_routes(state))
    }

    fn v1_core_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/dashboard",
                get(dashboard).route_layer(require_api.clone()),
            )
            .route(
                "/v1/auth/refresh",
                post(refresh_api_key).route_layer(require_api.clone()),
            )
            .route(
                "/v1/config",
                get(get_config_snapshot)
                    .patch(settings_patch)
                    .route_layer(require_api.clone()),
            )
            .route(
                "/v1/fs/browse",
                get(browse_filesystem).route_layer(require_api.clone()),
            )
            .route("/v1/logs/stream", get(stream_logs).route_layer(require_api))
    }

    fn v1_indexer_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        Router::new()
            .merge(Self::v1_indexer_definition_routes(state))
            .merge(Self::v1_indexer_tag_routes(state))
            .merge(Self::v1_indexer_health_notification_routes(state))
            .merge(Self::v1_indexer_secret_routes(state))
            .merge(Self::v1_indexer_backup_routes(state))
            .merge(Self::v1_indexer_category_mapping_routes(state))
            .merge(Self::v1_indexer_conflict_routes(state))
            .merge(Self::v1_indexer_torznab_instance_routes(state))
            .merge(Self::v1_indexer_import_job_routes(state))
            .merge(Self::v1_indexer_search_request_routes(state))
            .merge(Self::v1_indexer_search_profile_routes(state))
            .merge(Self::v1_indexer_policy_routes(state))
            .merge(Self::v1_indexer_routing_policy_routes(state))
            .merge(Self::v1_indexer_rate_limit_routes(state))
            .merge(Self::v1_indexer_instance_routes(state))
    }

    fn v1_indexer_definition_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/definitions",
                get(indexer_handlers::list_indexer_definitions),
            )
            .route(
                "/v1/indexers/definitions/import/cardigann",
                post(indexer_handlers::import_cardigann_definition),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_tag_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/tags",
                get(indexer_handlers::list_tags)
                    .post(indexer_handlers::create_tag)
                    .patch(indexer_handlers::update_tag)
                    .delete(indexer_handlers::delete_tag),
            )
            .route(
                "/v1/indexers/tags/{tag_key}",
                delete(indexer_handlers::delete_tag_by_key),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_conflict_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/conflicts",
                get(indexer_handlers::list_source_metadata_conflicts)
                    .patch(indexer_handlers::resolve_source_metadata_conflict),
            )
            .route(
                "/v1/indexers/conflicts/reopen",
                post(indexer_handlers::reopen_source_metadata_conflict),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_health_notification_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/health-notifications",
                get(indexer_handlers::list_health_notification_hooks)
                    .post(indexer_handlers::create_health_notification_hook)
                    .patch(indexer_handlers::update_health_notification_hook)
                    .delete(indexer_handlers::delete_health_notification_hook),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_secret_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/secrets",
                get(indexer_handlers::list_secret_metadata)
                    .post(indexer_handlers::create_secret)
                    .patch(indexer_handlers::rotate_secret)
                    .delete(indexer_handlers::revoke_secret),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_backup_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/backup/export",
                get(indexer_handlers::export_indexer_backup),
            )
            .route(
                "/v1/indexers/backup/restore",
                post(indexer_handlers::restore_indexer_backup),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_category_mapping_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/category-mappings/tracker",
                post(indexer_handlers::upsert_tracker_category_mapping)
                    .delete(indexer_handlers::delete_tracker_category_mapping),
            )
            .route(
                "/v1/indexers/category-mappings/media-domains",
                post(indexer_handlers::upsert_media_domain_mapping)
                    .delete(indexer_handlers::delete_media_domain_mapping),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_torznab_instance_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/torznab-instances",
                post(indexer_handlers::create_torznab_instance),
            )
            .route(
                "/v1/indexers/torznab-instances/{torznab_instance_public_id}/rotate",
                patch(indexer_handlers::rotate_torznab_instance_key),
            )
            .route(
                "/v1/indexers/torznab-instances/{torznab_instance_public_id}/state",
                put(indexer_handlers::set_torznab_instance_state),
            )
            .route(
                "/v1/indexers/torznab-instances/{torznab_instance_public_id}",
                delete(indexer_handlers::delete_torznab_instance),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_import_job_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/import-jobs",
                post(indexer_handlers::create_import_job),
            )
            .route(
                "/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api",
                post(indexer_handlers::run_import_job_prowlarr_api),
            )
            .route(
                "/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup",
                post(indexer_handlers::run_import_job_prowlarr_backup),
            )
            .route(
                "/v1/indexers/import-jobs/{import_job_public_id}/status",
                get(indexer_handlers::get_import_job_status),
            )
            .route(
                "/v1/indexers/import-jobs/{import_job_public_id}/results",
                get(indexer_handlers::list_import_job_results),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_search_request_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/search-requests",
                post(indexer_handlers::create_search_request),
            )
            .route(
                "/v1/indexers/search-requests/{search_request_public_id}/cancel",
                post(indexer_handlers::cancel_search_request),
            )
            .route(
                "/v1/indexers/search-requests/{search_request_public_id}/pages",
                get(indexer_handlers::list_search_pages),
            )
            .route(
                "/v1/indexers/search-requests/{search_request_public_id}/pages/{page_number}",
                get(indexer_handlers::get_search_page),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_search_profile_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/search-profiles",
                post(indexer_handlers::create_search_profile),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}",
                patch(indexer_handlers::update_search_profile),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/default",
                post(indexer_handlers::set_search_profile_default),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/default-domain",
                put(indexer_handlers::set_search_profile_default_domain),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/media-domains",
                put(indexer_handlers::set_search_profile_domain_allowlist),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets",
                post(indexer_handlers::add_search_profile_policy_set)
                    .delete(indexer_handlers::remove_search_profile_policy_set),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/indexers/allow",
                put(indexer_handlers::set_search_profile_indexer_allow),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/indexers/block",
                put(indexer_handlers::set_search_profile_indexer_block),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/tags/allow",
                put(indexer_handlers::set_search_profile_tag_allow),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/tags/block",
                put(indexer_handlers::set_search_profile_tag_block),
            )
            .route(
                "/v1/indexers/search-profiles/{search_profile_public_id}/tags/prefer",
                put(indexer_handlers::set_search_profile_tag_prefer),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_policy_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/policies",
                post(indexer_handlers::create_policy_set),
            )
            .route(
                "/v1/indexers/policies/{policy_set_public_id}",
                patch(indexer_handlers::update_policy_set),
            )
            .route(
                "/v1/indexers/policies/{policy_set_public_id}/enable",
                post(indexer_handlers::enable_policy_set),
            )
            .route(
                "/v1/indexers/policies/{policy_set_public_id}/disable",
                post(indexer_handlers::disable_policy_set),
            )
            .route(
                "/v1/indexers/policies/reorder",
                post(indexer_handlers::reorder_policy_sets),
            )
            .route(
                "/v1/indexers/policies/{policy_set_public_id}/rules",
                post(indexer_handlers::create_policy_rule),
            )
            .route(
                "/v1/indexers/policies/{policy_set_public_id}/rules/reorder",
                post(indexer_handlers::reorder_policy_rules),
            )
            .route(
                "/v1/indexers/policies/rules/{policy_rule_public_id}/enable",
                post(indexer_handlers::enable_policy_rule),
            )
            .route(
                "/v1/indexers/policies/rules/{policy_rule_public_id}/disable",
                post(indexer_handlers::disable_policy_rule),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_routing_policy_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/routing-policies",
                post(indexer_handlers::create_routing_policy),
            )
            .route(
                "/v1/indexers/routing-policies/{routing_policy_public_id}",
                get(indexer_handlers::get_routing_policy),
            )
            .route(
                "/v1/indexers/routing-policies/{routing_policy_public_id}/params",
                post(indexer_handlers::set_routing_policy_param),
            )
            .route(
                "/v1/indexers/routing-policies/{routing_policy_public_id}/secrets",
                post(indexer_handlers::bind_routing_policy_secret),
            )
            .route(
                "/v1/indexers/routing-policies/{routing_policy_public_id}/rate-limit",
                put(indexer_handlers::set_routing_policy_rate_limit),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_rate_limit_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/rate-limits",
                post(indexer_handlers::create_rate_limit_policy),
            )
            .route(
                "/v1/indexers/rate-limits/{rate_limit_policy_public_id}",
                patch(indexer_handlers::update_rate_limit_policy)
                    .delete(indexer_handlers::delete_rate_limit_policy),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/rate-limit",
                put(indexer_handlers::set_indexer_instance_rate_limit),
            )
            .route_layer(require_api)
    }

    fn v1_indexer_instance_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
            .route(
                "/v1/indexers/instances",
                post(indexer_handlers::create_indexer_instance),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}",
                patch(indexer_handlers::update_indexer_instance),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/media-domains",
                put(indexer_handlers::set_indexer_instance_media_domains),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/tags",
                put(indexer_handlers::set_indexer_instance_tags),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/fields/value",
                patch(indexer_handlers::set_indexer_instance_field_value),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/fields/secret",
                patch(indexer_handlers::bind_indexer_instance_field_secret),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/cf-state",
                get(indexer_handlers::get_indexer_instance_cf_state),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/connectivity-profile",
                get(indexer_handlers::get_indexer_connectivity_profile),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/reputation",
                get(indexer_handlers::get_indexer_source_reputation),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/health-events",
                get(indexer_handlers::get_indexer_health_events),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/rss",
                get(indexer_handlers::get_indexer_rss_subscription)
                    .put(indexer_handlers::put_indexer_rss_subscription),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/rss/items",
                get(indexer_handlers::get_indexer_rss_items)
                    .post(indexer_handlers::mark_indexer_rss_item_seen),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/cf-state/reset",
                post(indexer_handlers::reset_indexer_instance_cf_state),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/test/prepare",
                post(indexer_handlers::prepare_indexer_instance_test),
            )
            .route(
                "/v1/indexers/instances/{indexer_instance_public_id}/test/finalize",
                post(indexer_handlers::finalize_indexer_instance_test),
            )
            .route_layer(require_api)
    }

    fn v1_torrent_routes(state: &Arc<ApiState>) -> Router<Arc<ApiState>> {
        let require_api = middleware::from_fn_with_state(state.clone(), require_api_key);

        Router::new()
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
    pub async fn serve(self, addr: SocketAddr) -> ApiServerResult<()> {
        tracing::info!(addr = %addr, "Starting API listener");
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|source| ApiServerError::Bind { addr, source })?;
        axum::serve(
            listener,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|source| ApiServerError::Serve { source })?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn router(&self) -> &Router {
        &self.router
    }
}
