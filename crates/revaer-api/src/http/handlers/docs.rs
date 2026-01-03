//! Documentation endpoints.

use std::sync::Arc;

use axum::{Json, extract::State};
use serde_json::Value;

use crate::app::state::ApiState;

pub(crate) async fn openapi_document_handler(State(state): State<Arc<ApiState>>) -> Json<Value> {
    Json((*state.openapi_document).clone())
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

    #[derive(Clone, Default)]
    struct StubConfig;

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Ok(sample_snapshot().app_profile)
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
                operation: "docs.apply_changeset",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Ok(sample_snapshot())
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

    fn sample_snapshot() -> ConfigSnapshot {
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
                mode: AppMode::Setup,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
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

    #[tokio::test]
    async fn openapi_handler_clones_embedded_document() -> Result<()> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig);
        let telemetry = Metrics::new()?;
        let document = Arc::new(serde_json::json!({"hello": "world"}));
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::clone(&document),
            EventBus::new(),
            None,
        ));

        let Json(body) = openapi_document_handler(State(state.clone())).await;
        assert_eq!(body, *document);
        assert_eq!(
            Arc::strong_count(&document),
            2,
            "document should be cloned per request"
        );
        Ok(())
    }

    #[tokio::test]
    async fn stub_config_exposes_expected_behavior() -> Result<()> {
        let config = StubConfig;
        let token = config
            .issue_setup_token(Duration::from_secs(30), "tester")
            .await?;
        assert_eq!(token.plaintext, "token");

        config.validate_setup_token("token").await?;
        config.consume_setup_token("token").await?;

        let err = config
            .apply_changeset("actor", "reason", SettingsChangeset::default())
            .await
            .err()
            .ok_or_else(|| anyhow!("expected apply error"))?;
        assert!(matches!(err, ConfigError::Io { .. }));

        let auth = config.authenticate_api_key("id", "secret").await?;
        assert!(auth.is_none());
        Ok(())
    }
}
