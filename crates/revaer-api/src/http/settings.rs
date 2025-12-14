//! Settings/configuration endpoints.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
};
use revaer_config::{ConfigError, ConfigSnapshot, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use tracing::error;

use crate::app::state::ApiState;
use crate::http::auth::{AuthContext, map_config_error};
use crate::http::errors::ApiError;
use crate::models::ProblemInvalidParam;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFacade;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use chrono::Utc;
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigSnapshot,
        EngineProfile, FsPolicy, SettingsChangeset, SetupToken, normalize_engine_profile,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig {
        snapshot: ConfigSnapshot,
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(self.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(&self, _ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            Ok(SetupToken {
                plaintext: "token".into(),
                expires_at: Utc::now(),
            })
        }

        async fn validate_setup_token(&self, _token: &str) -> Result<()> {
            Ok(())
        }

        async fn consume_setup_token(&self, _token: &str) -> Result<()> {
            Ok(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            Err(anyhow!("not implemented"))
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            Ok(self.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            Ok(None)
        }
    }

    fn sample_snapshot() -> ConfigSnapshot {
        let engine_profile = EngineProfile {
            id: Uuid::nil(),
            implementation: "stub".into(),
            listen_port: None,
            listen_interfaces: Vec::new(),
            ipv6_mode: "disabled".into(),
            anonymous_mode: false.into(),
            force_proxy: false.into(),
            prefer_rc4: false.into(),
            allow_multiple_connections_per_ip: false.into(),
            enable_outgoing_utp: false.into(),
            enable_incoming_utp: false.into(),
            dht: true,
            encryption: "prefer".into(),
            max_active: Some(1),
            max_download_bps: None,
            max_upload_bps: None,
            sequential_default: false,
            resume_dir: "/tmp".into(),
            download_root: "/tmp/downloads".into(),
            tracker: json!([]),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: json!({}),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        };
        ConfigSnapshot {
            revision: 11,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode: AppMode::Active,
                version: 1,
                http_port: 3030,
                bind_addr: "127.0.0.1".parse().expect("bind addr"),
                telemetry: json!({}),
                features: json!({}),
                immutable_keys: json!([]),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::nil(),
                library_root: "/tmp/library".into(),
                extract: false,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: json!([]),
                cleanup_drop: json!([]),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: json!([]),
            },
        }
    }

    #[tokio::test]
    async fn settings_patch_rejects_setup_token_context() {
        let state = Arc::new(ApiState::new(
            Arc::new(StubConfig {
                snapshot: sample_snapshot(),
            }),
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::new(),
            None,
        ));
        let context = AuthContext::SetupToken("token".into());
        let result = settings_patch(
            State(state),
            Extension(context),
            Json(SettingsChangeset::default()),
        )
        .await;
        assert!(result.is_err(), "setup tokens cannot patch settings");
    }

    #[tokio::test]
    async fn well_known_returns_snapshot() {
        let snapshot = sample_snapshot();
        let state = Arc::new(ApiState::new(
            Arc::new(StubConfig {
                snapshot: snapshot.clone(),
            }),
            Metrics::new().expect("metrics"),
            Arc::new(json!({})),
            EventBus::new(),
            None,
        ));

        let Json(body) = well_known(State(state)).await.expect("well_known");
        assert_eq!(body.revision, snapshot.revision);
        assert_eq!(
            body.engine_profile.listen_port,
            snapshot.engine_profile.listen_port
        );
        assert_eq!(
            body.engine_profile_effective.network.listen_port,
            snapshot.engine_profile_effective.network.listen_port
        );
    }

    #[test]
    fn invalid_params_are_projected_from_config_error() {
        let immutables = invalid_params_for_config_error(&ConfigError::ImmutableField {
            section: "app_profile".into(),
            field: "http_port".into(),
        });
        assert_eq!(immutables[0].pointer, "/app_profile/http_port");
        assert!(
            immutables[0].message.contains("immutable"),
            "immutable error should be described"
        );

        let invalids = invalid_params_for_config_error(&ConfigError::InvalidField {
            section: "fs_policy".into(),
            field: "move_mode".into(),
            message: "bad value".into(),
        });
        assert_eq!(invalids[0].pointer, "/fs_policy/move_mode");
        assert_eq!(invalids[0].message, "bad value");

        let unknowns = invalid_params_for_config_error(&ConfigError::UnknownField {
            section: "engine_profile".into(),
            field: "unexpected".into(),
        });
        assert_eq!(unknowns[0].pointer, "/engine_profile/unexpected");
        assert!(
            unknowns[0].message.contains("unknown field"),
            "unknown field should be described"
        );
    }
}
