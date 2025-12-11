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
        SettingsChangeset, SetupToken,
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
            engine_profile: EngineProfile {
                id: Uuid::nil(),
                implementation: "stub".into(),
                listen_port: None,
                dht: false,
                encryption: "prefer".into(),
                max_active: None,
                max_download_bps: None,
                max_upload_bps: None,
                sequential_default: false,
                resume_dir: "/tmp".into(),
                download_root: "/tmp/downloads".into(),
                tracker: json!([]),
            },
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
