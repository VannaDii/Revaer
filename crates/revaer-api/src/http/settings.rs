//! Settings/configuration endpoints.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, State},
    http::StatusCode,
};
use revaer_config::{ConfigError, ConfigSnapshot, SettingsChangeset};
use revaer_events::Event as CoreEvent;
use tracing::error;

use crate::app::state::ApiState;
use crate::http::auth::{AuthContext, map_config_error};
use crate::http::errors::ApiError;
use crate::models::{FactoryResetRequest, ProblemInvalidParam};

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
        .map_err(|err| map_config_error(&err, "failed to apply settings changes"))?;

    let snapshot = state.config.snapshot().await.map_err(|err| {
        error!(error = %err, "failed to load configuration snapshot");
        ApiError::internal("failed to load configuration snapshot")
    })?;

    state.publish_event(CoreEvent::SettingsChanged {
        description: format!("settings_patch revision {}", snapshot.revision),
    });

    Ok(Json(snapshot))
}

pub(crate) async fn factory_reset(
    State(state): State<Arc<ApiState>>,
    Extension(context): Extension<AuthContext>,
    Json(request): Json<FactoryResetRequest>,
) -> Result<StatusCode, ApiError> {
    match context {
        AuthContext::ApiKey { .. } => {}
        AuthContext::SetupToken(_) => {
            return Err(ApiError::unauthorized(
                "setup authentication context cannot factory reset",
            ));
        }
    }

    if request.confirm.trim() != "factory reset" {
        return Err(ApiError::bad_request(
            "confirmation phrase must be 'factory reset'",
        ));
    }

    state.config.factory_reset().await.map_err(|err| {
        error!(error = %err, "factory reset failed");
        ApiError::internal("factory reset failed").with_context_field("error", err.to_string())
    })?;

    Ok(StatusCode::NO_CONTENT)
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
            message: "field is immutable".to_string(),
        }],
        ConfigError::InvalidField {
            section,
            field,
            reason,
            ..
        } => vec![ProblemInvalidParam {
            pointer: crate::http::auth::pointer_for(section, field),
            message: (*reason).to_string(),
        }],
        ConfigError::UnknownField { section, field } => vec![ProblemInvalidParam {
            pointer: crate::http::auth::pointer_for(section, field),
            message: "unknown field".to_string(),
        }],
        _ => Vec::new(),
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
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        EngineProfile, FsPolicy, SettingsChangeset, SetupToken, TelemetryConfig,
        engine_profile::{AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig},
        normalize_engine_profile,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig {
        snapshot: ConfigSnapshot,
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(self.snapshot.app_profile.clone())
        }

        async fn issue_setup_token(
            &self,
            _ttl: Duration,
            _issued_by: &str,
        ) -> ConfigResult<SetupToken> {
            Ok(SetupToken {
                plaintext: "token".into(),
                expires_at: Utc::now(),
            })
        }

        async fn validate_setup_token(&self, _token: &str) -> ConfigResult<()> {
            Ok(())
        }

        async fn consume_setup_token(&self, _token: &str) -> ConfigResult<()> {
            Ok(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            Err(ConfigError::Io {
                operation: "settings.apply_changeset",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Ok(self.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(true)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Ok(())
        }
    }

    fn sample_snapshot() -> Result<ConfigSnapshot> {
        let bind_addr = "127.0.0.1"
            .parse()
            .map_err(|_| anyhow!("invalid bind address"))?;
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
            seed_ratio_limit: None,
            seed_time_limit: None,
            connections_limit: None,
            connections_limit_per_torrent: None,
            unchoke_slots: None,
            half_open_limit: None,
            stats_interval_ms: None,
            alt_speed: AltSpeedConfig::default(),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: "/tmp".into(),
            download_root: "/tmp/downloads".into(),
            storage_mode: EngineProfile::default_storage_mode(),
            use_partfile: EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: EngineProfile::default_coalesce_reads(),
            coalesce_writes: EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: EngineProfile::default_use_disk_cache_pool(),
            tracker: TrackerConfig::default(),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: IpFilterConfig::default(),
            peer_classes: PeerClassesConfig::default(),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        };
        Ok(ConfigSnapshot {
            revision: 11,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode: AppMode::Active,
                version: 1,
                http_port: 3030,
                bind_addr,
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
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
                cleanup_keep: Vec::new(),
                cleanup_drop: Vec::new(),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: Vec::new(),
            },
        })
    }

    #[tokio::test]
    async fn settings_patch_rejects_setup_token_context() -> Result<()> {
        let state = Arc::new(ApiState::new(
            Arc::new(StubConfig {
                snapshot: sample_snapshot()?,
            }),
            Metrics::new()?,
            Arc::new(serde_json::json!({})),
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
        Ok(())
    }

    #[tokio::test]
    async fn well_known_returns_snapshot() -> Result<()> {
        let snapshot = sample_snapshot()?;
        let state = Arc::new(ApiState::new(
            Arc::new(StubConfig {
                snapshot: snapshot.clone(),
            }),
            Metrics::new()?,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            None,
        ));

        let Json(body) = well_known(State(state)).await?;
        assert_eq!(body.revision, snapshot.revision);
        assert_eq!(
            body.engine_profile.listen_port,
            snapshot.engine_profile.listen_port
        );
        assert_eq!(
            body.engine_profile_effective.network.listen_port,
            snapshot.engine_profile_effective.network.listen_port
        );
        Ok(())
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
            value: None,
            reason: "bad value",
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
