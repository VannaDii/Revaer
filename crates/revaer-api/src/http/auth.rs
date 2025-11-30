//! Authentication and authorization middleware for the HTTP layer.

use std::sync::Arc;

use axum::{extract::State, http::Request, middleware::Next, response::Response};
use revaer_config::AppMode;
use revaer_telemetry::record_app_mode;
use tracing::{error, info, warn};

use crate::http::constants::{HEADER_API_KEY, HEADER_API_KEY_LEGACY, HEADER_SETUP_TOKEN};
use crate::http::errors::ApiError;
use crate::http::settings::invalid_params_for_config_error;
use crate::rate_limit::insert_rate_limit_headers;
use crate::state::ApiState;

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

pub(crate) fn map_config_error(err: anyhow::Error, context: &'static str) -> ApiError {
    match err.downcast::<revaer_config::ConfigError>() {
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
