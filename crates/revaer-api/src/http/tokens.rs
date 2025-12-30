//! API token refresh endpoints.
//!
//! # Design
//! - Refresh only extends expiry for authenticated API keys; no secret rotation.
//! - Anonymous or setup contexts are rejected to avoid false refreshes.
//! - Errors are surfaced via `ApiError` with stable messages.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
};
use chrono::{Duration as ChronoDuration, Utc};
use revaer_config::{ApiKeyPatch, SettingsChangeset};

use crate::app::state::ApiState;
use crate::http::auth::{AuthContext, map_config_error};
use crate::http::constants::API_KEY_TTL_DAYS;
use crate::http::errors::ApiError;
use crate::models::ApiKeyRefreshResponse;

pub(crate) async fn refresh_api_key(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
) -> Result<Json<ApiKeyRefreshResponse>, ApiError> {
    let key_id = match context {
        AuthContext::ApiKey { key_id } => key_id,
        AuthContext::Anonymous | AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized("API key required for token refresh"));
        }
    };

    let expires_at = Utc::now() + ChronoDuration::days(API_KEY_TTL_DAYS);
    let mut changeset = SettingsChangeset::default();
    changeset.api_keys.push(ApiKeyPatch::Upsert {
        key_id: key_id.clone(),
        label: None,
        enabled: None,
        expires_at: Some(expires_at),
        secret: None,
        rate_limit: None,
    });

    state
        .config
        .apply_changeset(&key_id, "api_key_refresh", changeset)
        .await
        .map_err(|err| map_config_error(&err, "failed to refresh api key"))?;

    Ok(Json(ApiKeyRefreshResponse {
        api_key_expires_at: expires_at.to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::auth::AuthContext;
    use anyhow::Result;
    use async_trait::async_trait;
    use axum::http::StatusCode;
    use revaer_config::{
        ApiKeyAuth, ApiKeyPatch, AppAuthMode, AppMode, AppProfile, AppliedChanges, ConfigError,
        ConfigResult, ConfigSnapshot, SettingsChangeset, SetupToken, TelemetryConfig,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    #[derive(Clone)]
    struct RecordingConfig {
        calls: Arc<Mutex<Vec<SettingsChangeset>>>,
    }

    #[async_trait]
    impl crate::config::ConfigFacade for RecordingConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "demo".to_string(),
                mode: AppMode::Active,
                auth_mode: AppAuthMode::ApiKey,
                version: 1,
                http_port: 7070,
                bind_addr: "127.0.0.1"
                    .parse()
                    .map_err(|_| ConfigError::InvalidBindAddr {
                        value: "127.0.0.1".to_string(),
                    })?,
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
            changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            self.calls.lock().await.push(changeset);
            Ok(AppliedChanges {
                revision: 1,
                app_profile: None,
                engine_profile: None,
                fs_policy: None,
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
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(true)
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

    fn api_state(config: RecordingConfig) -> Result<Arc<ApiState>> {
        let metrics = Metrics::new()?;
        Ok(Arc::new(ApiState::new(
            Arc::new(config),
            metrics,
            Arc::new(json!({})),
            EventBus::with_capacity(2),
            None,
        )))
    }

    #[tokio::test]
    async fn refresh_api_key_updates_expiry() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let state = api_state(RecordingConfig {
            calls: calls.clone(),
        })?;
        let Json(response) = refresh_api_key(
            State(state),
            Extension(AuthContext::ApiKey {
                key_id: "demo".to_string(),
            }),
        )
        .await?;

        assert!(
            !response.api_key_expires_at.is_empty(),
            "refresh should return an expiry"
        );

        let patch = {
            let guard = calls.lock().await;
            assert_eq!(guard.len(), 1);
            guard[0]
                .api_keys
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("expected api key patch"))?
        };
        match patch {
            ApiKeyPatch::Upsert {
                key_id, expires_at, ..
            } => {
                assert_eq!(key_id, "demo");
                assert!(expires_at.is_some());
            }
            ApiKeyPatch::Delete { .. } => {
                return Err(anyhow::anyhow!("expected upsert patch"));
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn refresh_api_key_rejects_anonymous() -> Result<()> {
        let state = api_state(RecordingConfig {
            calls: Arc::new(Mutex::new(Vec::new())),
        })?;
        let err = refresh_api_key(State(state), Extension(AuthContext::Anonymous))
            .await
            .unwrap_err();
        assert_eq!(err.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
