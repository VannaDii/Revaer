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
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigSnapshot, EngineProfile, FsPolicy,
        SettingsChangeset, SetupToken, normalize_engine_profile,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone, Default)]
    struct StubConfig;

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            Ok(sample_snapshot().app_profile)
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
            Ok(sample_snapshot())
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
            alt_speed: json!({}),
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
            revision: 1,
            app_profile: AppProfile {
                id: Uuid::nil(),
                instance_name: "test".into(),
                mode: AppMode::Setup,
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
    async fn openapi_handler_clones_embedded_document() {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig);
        let telemetry = Metrics::new().expect("metrics");
        let document = Arc::new(json!({"hello": "world"}));
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
    }

    #[tokio::test]
    async fn stub_config_exposes_expected_behavior() {
        let config = StubConfig;
        let token = config
            .issue_setup_token(Duration::from_secs(30), "tester")
            .await
            .expect("token issued");
        assert_eq!(token.plaintext, "token");

        config
            .validate_setup_token("token")
            .await
            .expect("validation should succeed");
        config
            .consume_setup_token("token")
            .await
            .expect("consumption should succeed");

        let err = config
            .apply_changeset(
                "actor",
                "reason",
                SettingsChangeset {
                    app_profile: Some(json!({})),
                    engine_profile: Some(json!({})),
                    fs_policy: Some(json!({})),
                    secrets: Vec::new(),
                    api_keys: Vec::new(),
                },
            )
            .await
            .expect_err("apply should be unimplemented");
        assert!(err.to_string().contains("not implemented"));

        let auth = config
            .authenticate_api_key("id", "secret")
            .await
            .expect("authentication call succeeds");
        assert!(auth.is_none());
    }
}
