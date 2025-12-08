//! Setup bootstrap endpoints.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Extension, State},
};
use revaer_config::{AppMode, ConfigSnapshot, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use revaer_telemetry::record_app_mode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tracing::{error, warn};

use crate::app::state::ApiState;
use crate::http::auth::{AuthContext, extract_setup_token, map_config_error};
use crate::http::errors::ApiError;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct SetupStartRequest {
    pub(crate) issued_by: Option<String>,
    pub(crate) ttl_seconds: Option<u64>,
}

#[derive(Serialize)]
pub(crate) struct SetupStartResponse {
    pub(crate) token: String,
    pub(crate) expires_at: chrono::DateTime<chrono::Utc>,
}

pub(crate) async fn setup_start(
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

pub(crate) async fn setup_complete(
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
