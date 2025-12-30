//! Setup bootstrap endpoints.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Extension, State},
};
use chrono::{Duration as ChronoDuration, Utc};
use revaer_config::{ApiKeyPatch, AppMode, ConfigSnapshot, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use revaer_telemetry::record_app_mode;
use tracing::{error, warn};
use uuid::Uuid;

use crate::app::state::ApiState;
use crate::http::auth::{AuthContext, extract_setup_token, map_config_error};
use crate::http::constants::API_KEY_TTL_DAYS;
use crate::http::errors::ApiError;
use crate::models::{SetupCompleteResponse, SetupStartRequest, SetupStartResponse};

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
        expires_at: token.expires_at.to_rfc3339(),
    }))
}

pub(crate) async fn setup_complete(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    Json(mut changeset): Json<SettingsChangeset>,
) -> Result<Json<SetupCompleteResponse>, ApiError> {
    let token = extract_setup_token(context)?;
    ensure_valid_setup_token(&state, &token).await?;
    ensure_active_app_profile(&state, &mut changeset).await?;
    let bootstrap_key = ensure_bootstrap_api_key(&mut changeset);

    let snapshot = apply_setup_changes(&state, changeset, &token).await?;

    state.publish_event(CoreEvent::SettingsChanged {
        description: format!("setup_complete revision {}", snapshot.revision),
    });

    let snapshot_value = serde_json::to_value(&snapshot).map_err(|err| {
        error!(error = %err, "failed to serialize setup completion snapshot");
        ApiError::internal("failed to serialize setup snapshot")
    })?;

    Ok(Json(SetupCompleteResponse {
        snapshot: snapshot_value,
        api_key: format!("{}:{}", bootstrap_key.key_id, bootstrap_key.secret),
        api_key_expires_at: bootstrap_key.expires_at.to_rfc3339(),
    }))
}

struct BootstrapApiKey {
    key_id: String,
    secret: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

fn ensure_bootstrap_api_key(changeset: &mut SettingsChangeset) -> BootstrapApiKey {
    let expires_at = Utc::now() + ChronoDuration::days(API_KEY_TTL_DAYS);
    for patch in &mut changeset.api_keys {
        if let ApiKeyPatch::Upsert {
            key_id,
            secret: Some(secret),
            expires_at: patch_expires_at,
            ..
        } = patch
        {
            if secret.trim().is_empty() {
                continue;
            }
            if patch_expires_at.is_none() {
                *patch_expires_at = Some(expires_at);
            }
            return BootstrapApiKey {
                key_id: key_id.clone(),
                secret: secret.clone(),
                expires_at,
            };
        }
    }

    let key_id = Uuid::new_v4().simple().to_string();
    let secret = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    changeset.api_keys.push(ApiKeyPatch::Upsert {
        key_id: key_id.clone(),
        label: Some("bootstrap".to_string()),
        enabled: Some(true),
        expires_at: Some(expires_at),
        secret: Some(secret.clone()),
        rate_limit: None,
    });

    BootstrapApiKey {
        key_id,
        secret,
        expires_at,
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

async fn ensure_active_app_profile(
    state: &ApiState,
    changeset: &mut SettingsChangeset,
) -> Result<(), ApiError> {
    let mut profile = match changeset.app_profile.take() {
        Some(profile) => profile,
        None => state.config.get_app_profile().await.map_err(|err| {
            error!(error = %err, "failed to load app profile for setup completion");
            ApiError::internal("failed to load app profile")
        })?,
    };
    profile.mode = AppMode::Active;
    changeset.app_profile = Some(profile);
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
        .map_err(|err| map_config_error(&err, "failed to apply setup changes"))?;

    if let Err(err) = state.config.consume_setup_token(token).await {
        error!(error = %err, "failed to consume setup token after completion");
        return Err(ApiError::internal("failed to finalize setup"));
    }

    state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })
}
