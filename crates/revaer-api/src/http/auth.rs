//! Authentication and authorization middleware for the HTTP layer.

use std::sync::Arc;

use axum::{extract::State, http::Request, middleware::Next, response::Response};
use revaer_config::AppMode;
use revaer_telemetry::record_app_mode;
use tracing::{error, info, warn};

use crate::app::state::ApiState;
use crate::http::constants::{HEADER_API_KEY, HEADER_API_KEY_LEGACY, HEADER_SETUP_TOKEN};
use crate::http::errors::ApiError;
use crate::http::rate_limit::insert_rate_limit_headers;
use crate::http::settings::invalid_params_for_config_error;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::{get, post},
    };
    use revaer_config::{
        ApiKeyAuth, ApiKeyRateLimit, AppMode, AppProfile, AppliedChanges, ConfigError,
        ConfigResult, ConfigSnapshot, SettingsChangeset, SetupToken, TelemetryConfig,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::{
        net::{IpAddr, Ipv4Addr},
        time::Duration,
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    #[derive(Clone)]
    struct MockConfig {
        mode: AppMode,
        api_auth: Option<ApiKeyAuth>,
        has_api_keys: bool,
    }

    #[async_trait]
    impl crate::config::ConfigFacade for MockConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".to_string(),
                mode: self.mode.clone(),
                version: 1,
                http_port: 8080,
                bind_addr: IpAddr::V4(Ipv4Addr::LOCALHOST),
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            })
        }

        async fn issue_setup_token(&self, _: Duration, _: &str) -> ConfigResult<SetupToken> {
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
            _: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "changeset".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Err(ConfigError::InvalidField {
                section: "config".to_string(),
                field: "snapshot".to_string(),
                value: None,
                reason: "not implemented",
            })
        }

        async fn authenticate_api_key(&self, _: &str, _: &str) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(self.api_auth.clone())
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(self.has_api_keys)
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

    fn router_with_state(state: &Arc<ApiState>) -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .with_state(state.clone())
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_api_key,
            ))
    }

    fn setup_router_with_state(state: &Arc<ApiState>) -> Router {
        Router::new()
            .route("/", get(|| async { "setup" }))
            .with_state(state.clone())
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_setup_token,
            ))
    }

    fn factory_reset_router_with_state(state: &Arc<ApiState>) -> Router {
        Router::new()
            .route("/", post(|| async { "ok" }))
            .with_state(state.clone())
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_factory_reset_auth,
            ))
    }

    fn api_state(
        mode: AppMode,
        auth: Option<ApiKeyAuth>,
        has_api_keys: bool,
    ) -> Result<Arc<ApiState>> {
        let metrics = Metrics::new()?;
        Ok(Arc::new(ApiState::new(
            Arc::new(MockConfig {
                mode,
                api_auth: auth,
                has_api_keys,
            }),
            metrics,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        )))
    }

    #[tokio::test]
    async fn require_api_key_rejects_missing_and_invalid() -> Result<()> {
        let state = api_state(AppMode::Active, None, true)?;
        let app = router_with_state(&state);

        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(crate::http::constants::HEADER_API_KEY, "invalid")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn require_api_key_allows_authenticated_request() -> Result<()> {
        let auth = ApiKeyAuth {
            key_id: "demo".to_string(),
            label: Some("label".to_string()),
            rate_limit: Some(ApiKeyRateLimit {
                burst: 5,
                replenish_period: Duration::from_secs(60),
            }),
        };
        let state = api_state(AppMode::Active, Some(auth), true)?;
        let app = router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(crate::http::constants::HEADER_API_KEY, "demo:secret-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn require_setup_token_enforces_mode_and_header() -> Result<()> {
        let state = api_state(AppMode::Setup, None, true)?;
        let app = setup_router_with_state(&state);

        let missing = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty())?)
            .await?;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let ok = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(crate::http::constants::HEADER_SETUP_TOKEN, "token-123")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(ok.status(), StatusCode::OK);

        // Active mode should reject setup tokens.
        let active_state = api_state(AppMode::Active, None, true)?;
        let active_app = setup_router_with_state(&active_state);
        let rejected = active_app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(crate::http::constants::HEADER_SETUP_TOKEN, "token-123")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(rejected.status(), StatusCode::CONFLICT);
        Ok(())
    }

    #[tokio::test]
    async fn require_factory_reset_allows_without_api_key_when_none_exist() -> Result<()> {
        let state = api_state(AppMode::Active, None, false)?;
        let app = factory_reset_router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn require_factory_reset_allows_invalid_api_key_when_none_exist() -> Result<()> {
        let state = api_state(AppMode::Active, None, false)?;
        let app = factory_reset_router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header(crate::http::constants::HEADER_API_KEY, "stale:token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn require_factory_reset_rejects_without_api_key_when_keys_exist() -> Result<()> {
        let state = api_state(AppMode::Active, None, true)?;
        let app = factory_reset_router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn require_factory_reset_rejects_invalid_api_key_when_keys_exist() -> Result<()> {
        let state = api_state(AppMode::Active, None, true)?;
        let app = factory_reset_router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header(crate::http::constants::HEADER_API_KEY, "stale:token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn require_factory_reset_accepts_valid_api_key() -> Result<()> {
        let auth = ApiKeyAuth {
            key_id: "demo".to_string(),
            label: Some("label".to_string()),
            rate_limit: Some(ApiKeyRateLimit {
                burst: 5,
                replenish_period: Duration::from_secs(60),
            }),
        };
        let state = api_state(AppMode::Active, Some(auth), true)?;
        let app = factory_reset_router_with_state(&state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header(crate::http::constants::HEADER_API_KEY, "demo:secret-token")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) enum AuthContext {
    SetupToken(String),
    ApiKey { key_id: String },
}

pub(crate) async fn require_setup_token(
    State(state): State<Arc<ApiState>>,
    mut req: Request<axum::body::Body>,
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

pub(crate) async fn require_api_key(
    State(state): State<Arc<ApiState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    info!("require_api_key start");
    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    if app.mode != AppMode::Active {
        return Err(ApiError::setup_required("system is still in setup mode"));
    }

    let api_key_raw = extract_api_key(&req)
        .ok_or_else(|| ApiError::unauthorized("missing API key header or query parameter"))?;

    let (key_id, secret) = api_key_raw
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

    let rate_snapshot = match state.enforce_rate_limit(&auth.key_id, auth.rate_limit.as_ref()) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return Err(ApiError::too_many_requests(
                "API key rate limit exceeded; try again later",
            )
            .with_rate_limit_headers(err.limit, 0, Some(err.retry_after)));
        }
    };

    req.extensions_mut().insert(AuthContext::ApiKey {
        key_id: auth.key_id,
    });

    let mut response = next.run(req).await;
    if let Some(snapshot) = rate_snapshot {
        insert_rate_limit_headers(
            response.headers_mut(),
            snapshot.limit,
            snapshot.remaining,
            None,
        );
    }
    Ok(response)
}

pub(crate) async fn require_factory_reset_auth(
    State(state): State<Arc<ApiState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    info!("require_factory_reset_auth start");
    let app = state.config.get_app_profile().await.map_err(|err| {
        error!(error = %err, "failed to load app profile");
        ApiError::internal("failed to load app profile")
    })?;
    record_app_mode(app.mode.as_str());

    if let Some(api_key_raw) = extract_api_key(&req) {
        let (key_id, secret) = api_key_raw
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
            let has_api_keys = state.config.has_api_keys().await.map_err(|err| {
                error!(error = %err, "failed to check API key inventory");
                ApiError::internal("failed to check API key inventory")
            })?;
            if has_api_keys {
                return Err(ApiError::unauthorized("invalid API key"));
            }
            warn!("factory reset allowed without API key because no keys exist");
            req.extensions_mut().insert(AuthContext::ApiKey {
                key_id: "bootstrap".to_string(),
            });
            return Ok(next.run(req).await);
        };

        let rate_snapshot = match state.enforce_rate_limit(&auth.key_id, auth.rate_limit.as_ref()) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                return Err(ApiError::too_many_requests(
                    "API key rate limit exceeded; try again later",
                )
                .with_rate_limit_headers(err.limit, 0, Some(err.retry_after)));
            }
        };

        req.extensions_mut().insert(AuthContext::ApiKey {
            key_id: auth.key_id,
        });

        let mut response = next.run(req).await;
        if let Some(snapshot) = rate_snapshot {
            insert_rate_limit_headers(
                response.headers_mut(),
                snapshot.limit,
                snapshot.remaining,
                None,
            );
        }
        return Ok(response);
    }

    let has_api_keys = state.config.has_api_keys().await.map_err(|err| {
        error!(error = %err, "failed to check API key inventory");
        ApiError::internal("failed to check API key inventory")
    })?;
    if has_api_keys {
        return Err(ApiError::unauthorized(
            "missing API key header or query parameter",
        ));
    }

    warn!("factory reset allowed without API key because no keys exist");
    req.extensions_mut().insert(AuthContext::ApiKey {
        key_id: "bootstrap".to_string(),
    });
    Ok(next.run(req).await)
}

pub(crate) fn extract_setup_token(context: AuthContext) -> Result<String, ApiError> {
    match context {
        AuthContext::SetupToken(token) => Ok(token),
        AuthContext::ApiKey { .. } => Err(ApiError::internal(
            "setup token required for this operation",
        )),
    }
}

pub(crate) fn extract_api_key(req: &Request<axum::body::Body>) -> Option<String> {
    let header_value = req
        .headers()
        .get(HEADER_API_KEY)
        .or_else(|| req.headers().get(HEADER_API_KEY_LEGACY))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(value) = header_value {
        return Some(value.to_string());
    }

    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some(value) = pair.strip_prefix("api_key=")
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub(crate) fn map_config_error(
    err: &revaer_config::ConfigError,
    context: &'static str,
) -> ApiError {
    warn!(error = %err, operation = context, "config error");
    let mut api_error =
        ApiError::config_invalid("configuration invalid").with_context_field("operation", context);
    let params = invalid_params_for_config_error(err);
    if !params.is_empty() {
        api_error = api_error.with_invalid_params(params);
    }
    if let revaer_config::ConfigError::InvalidField {
        value: Some(value), ..
    } = &err
    {
        api_error = api_error.with_context_field("value", value.clone());
    }
    api_error
}

pub(crate) fn pointer_for(section: &str, field: &str) -> String {
    let mut pointer = String::new();
    pointer.push('/');
    pointer.push_str(&encode_pointer_segment(section));

    if field != "<root>" && !field.is_empty() {
        pointer.push('/');
        pointer.push_str(&encode_pointer_segment(field));
    }

    pointer
}

pub(crate) fn encode_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
