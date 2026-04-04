//! Setup bootstrap endpoints.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::{Extension, State},
};
use chrono::{Duration as ChronoDuration, Utc};
use revaer_config::{ApiKeyPatch, AppAuthMode, AppMode, ConfigSnapshot, SettingsChangeset};
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
    let payload = match payload {
        Some(Json(payload)) => payload,
        None => SetupStartRequest::default(),
    };

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
    let auth_mode = changeset
        .app_profile
        .as_ref()
        .map_or(AppAuthMode::NoAuth, |profile| profile.auth_mode);
    let bootstrap_key = if auth_mode == AppAuthMode::ApiKey {
        Some(ensure_bootstrap_api_key(&mut changeset))
    } else {
        None
    };

    let snapshot = apply_setup_changes(&state, changeset, &token).await?;

    state.publish_event(CoreEvent::SettingsChanged {
        description: format!("setup_complete revision {}", snapshot.revision),
    });

    let (api_key, api_key_expires_at) = match bootstrap_key {
        Some(key) => (
            Some(format!("{}:{}", key.key_id, key.secret)),
            Some(key.expires_at.to_rfc3339()),
        ),
        None => (None, None),
    };

    Ok(Json(SetupCompleteResponse {
        snapshot,
        api_key,
        api_key_expires_at,
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
            *patch_expires_at = Some(expires_at);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::indexers::test_indexers;
    use crate::config::ConfigFacade;
    use crate::http::auth::AuthContext;
    use anyhow::Result;
    use async_trait::async_trait;
    use revaer_config::{
        ApiKeyAuth, AppAuthMode, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult,
        ConfigSnapshot, EngineProfile, FsPolicy, SettingsChangeset, SetupToken, TelemetryConfig,
        engine_profile::{AltSpeedConfig, IpFilterConfig, PeerClassesConfig, TrackerConfig},
        normalize_engine_profile,
        validate::default_local_networks,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig {
        state: Arc<Mutex<ConfigState>>,
    }

    struct ConfigState {
        app_profile: AppProfile,
        snapshot: ConfigSnapshot,
        validate_ok: bool,
        consume_ok: bool,
        apply_ok: bool,
    }

    impl StubConfig {
        fn new(app_mode: AppMode, auth_mode: AppAuthMode) -> Self {
            let snapshot = sample_snapshot(app_mode, auth_mode);
            let state = ConfigState {
                app_profile: snapshot.app_profile.clone(),
                snapshot,
                validate_ok: true,
                consume_ok: true,
                apply_ok: true,
            };
            Self {
                state: Arc::new(Mutex::new(state)),
            }
        }

        fn with_flags(self, validate_ok: bool, consume_ok: bool, apply_ok: bool) -> Self {
            if let Ok(mut state) = self.state.lock() {
                state.validate_ok = validate_ok;
                state.consume_ok = consume_ok;
                state.apply_ok = apply_ok;
            }
            self
        }
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            let state = self.state.lock().map_err(|_| ConfigError::Io {
                operation: "setup.get_app_profile",
                source: std::io::Error::other("poisoned"),
            })?;
            Ok(state.app_profile.clone())
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
            let state = self.state.lock().map_err(|_| ConfigError::Io {
                operation: "setup.validate_setup_token",
                source: std::io::Error::other("poisoned"),
            })?;
            if state.validate_ok {
                Ok(())
            } else {
                Err(ConfigError::Io {
                    operation: "setup.validate_setup_token",
                    source: std::io::Error::other("invalid token"),
                })
            }
        }

        async fn consume_setup_token(&self, _token: &str) -> ConfigResult<()> {
            let state = self.state.lock().map_err(|_| ConfigError::Io {
                operation: "setup.consume_setup_token",
                source: std::io::Error::other("poisoned"),
            })?;
            if state.consume_ok {
                Ok(())
            } else {
                Err(ConfigError::Io {
                    operation: "setup.consume_setup_token",
                    source: std::io::Error::other("consume failed"),
                })
            }
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            let state = self.state.lock().map_err(|_| ConfigError::Io {
                operation: "setup.apply_changeset",
                source: std::io::Error::other("poisoned"),
            })?;
            if state.apply_ok {
                Ok(AppliedChanges {
                    revision: state.snapshot.revision + 1,
                    app_profile: None,
                    engine_profile: None,
                    fs_policy: None,
                })
            } else {
                Err(ConfigError::Io {
                    operation: "setup.apply_changeset",
                    source: std::io::Error::other("apply failed"),
                })
            }
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            let state = self.state.lock().map_err(|_| ConfigError::Io {
                operation: "setup.snapshot",
                source: std::io::Error::other("poisoned"),
            })?;
            Ok(state.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(false)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Ok(())
        }
    }

    fn sample_snapshot(app_mode: AppMode, auth_mode: AppAuthMode) -> ConfigSnapshot {
        let bind_addr = std::net::IpAddr::from([127, 0, 0, 1]);
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
            dht: false,
            encryption: "prefer".into(),
            max_active: None,
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
            resume_dir: ".server_root/resume".into(),
            download_root: ".server_root/downloads".into(),
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
        ConfigSnapshot {
            revision: 1,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode: app_mode,
                auth_mode,
                version: 1,
                http_port: 3030,
                bind_addr,
                local_networks: default_local_networks(),
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            },
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(&engine_profile),
            fs_policy: FsPolicy {
                id: Uuid::nil(),
                library_root: ".server_root/library".into(),
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
        }
    }

    fn test_state(config: StubConfig) -> Result<Arc<ApiState>> {
        Ok(Arc::new(ApiState::new(
            Arc::new(config),
            test_indexers(),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        )))
    }

    #[tokio::test]
    async fn setup_start_returns_token_in_setup_mode() -> Result<()> {
        let state = test_state(StubConfig::new(AppMode::Setup, AppAuthMode::ApiKey))?;
        let Json(response) = setup_start(State(state), None).await?;
        assert_eq!(response.token, "token");
        Ok(())
    }

    #[tokio::test]
    async fn setup_start_rejects_active_mode() -> Result<()> {
        let state = test_state(StubConfig::new(AppMode::Active, AppAuthMode::ApiKey))?;
        let err = setup_start(State(state), None)
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("expected conflict"))?;
        assert_eq!(err.status(), axum::http::StatusCode::CONFLICT);
        Ok(())
    }

    #[test]
    fn ensure_bootstrap_api_key_reuses_existing_patch() {
        let mut changeset = SettingsChangeset::default();
        changeset.api_keys.push(revaer_config::ApiKeyPatch::Upsert {
            key_id: "existing".into(),
            label: None,
            enabled: Some(true),
            expires_at: None,
            secret: Some("secret".into()),
            rate_limit: None,
        });
        let key = ensure_bootstrap_api_key(&mut changeset);
        assert_eq!(key.key_id, "existing");
        assert_eq!(changeset.api_keys.len(), 1);
        match &changeset.api_keys[0] {
            revaer_config::ApiKeyPatch::Upsert { expires_at, .. } => {
                assert!(expires_at.is_some());
            }
            revaer_config::ApiKeyPatch::Delete { .. } => {
                panic!("expected upsert patch");
            }
        }
    }

    #[test]
    fn ensure_bootstrap_api_key_generates_when_missing() {
        let mut changeset = SettingsChangeset::default();
        let key = ensure_bootstrap_api_key(&mut changeset);
        assert_eq!(changeset.api_keys.len(), 1);
        match &changeset.api_keys[0] {
            revaer_config::ApiKeyPatch::Upsert { label, key_id, .. } => {
                assert_eq!(label.as_deref(), Some("bootstrap"));
                assert_eq!(key_id, &key.key_id);
            }
            revaer_config::ApiKeyPatch::Delete { .. } => {
                panic!("expected upsert patch");
            }
        }
    }

    #[tokio::test]
    async fn ensure_valid_setup_token_rejects_invalid_token() -> Result<()> {
        let config =
            StubConfig::new(AppMode::Setup, AppAuthMode::ApiKey).with_flags(false, true, true);
        let state = test_state(config)?;
        let err = ensure_valid_setup_token(&state, "bad")
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("expected validation error"))?;
        assert_eq!(err.status(), axum::http::StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn setup_complete_applies_and_returns_bootstrap_key() -> Result<()> {
        let state = test_state(StubConfig::new(AppMode::Setup, AppAuthMode::ApiKey))?;
        let changeset = SettingsChangeset {
            app_profile: Some(sample_snapshot(AppMode::Setup, AppAuthMode::ApiKey).app_profile),
            ..Default::default()
        };

        let Json(response) = setup_complete(
            State(state),
            Extension(AuthContext::SetupToken("token".into())),
            Json(changeset),
        )
        .await?;
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.api_key.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn apply_setup_changes_reports_consume_failure() -> Result<()> {
        let config =
            StubConfig::new(AppMode::Setup, AppAuthMode::ApiKey).with_flags(true, false, true);
        let state = test_state(config)?;
        let err = apply_setup_changes(&state, SettingsChangeset::default(), "token")
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("expected consume error"))?;
        assert_eq!(err.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }
}
