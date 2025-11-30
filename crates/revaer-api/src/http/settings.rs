//! Settings/configuration endpoints.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
};
use revaer_config::{ConfigError, ConfigSnapshot, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use tracing::error;

use crate::http::auth::{AuthContext, map_config_error};
use crate::http::errors::ApiError;
use crate::models::ProblemInvalidParam;
use crate::state::ApiState;

pub(crate) async fn settings_patch(
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

pub(crate) async fn well_known(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ConfigSnapshot>, ApiError> {
    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;
    Ok(Json(snapshot))
}

pub(crate) async fn get_config_snapshot(
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ConfigSnapshot>, ApiError> {
    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;
    Ok(Json(snapshot))
}

pub(crate) fn invalid_params_for_config_error(error: &ConfigError) -> Vec<ProblemInvalidParam> {
    match error {
        ConfigError::ImmutableField { section, field } => vec![ProblemInvalidParam {
            pointer: crate::http::auth::pointer_for(section, field),
            message: format!("field '{field}' in '{section}' is immutable"),
        }],
        ConfigError::InvalidField {
            section,
            field,
            message,
        } => vec![ProblemInvalidParam {
            pointer: crate::http::auth::pointer_for(section, field),
            message: message.clone(),
        }],
        ConfigError::UnknownField { section, field } => vec![ProblemInvalidParam {
            pointer: crate::http::auth::pointer_for(section, field),
            message: format!("unknown field '{field}' in '{section}'"),
        }],
    }
}
