//! Configuration facade abstraction for the API layer.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use revaer_config::{
    ApiKeyAuth, AppProfile, AppliedChanges, ConfigService, ConfigSnapshot, SettingsChangeset,
    SettingsFacade, SetupToken,
};

/// Trait defining the configuration backend used by the API layer.
#[async_trait]
pub trait ConfigFacade: Send + Sync {
    /// Retrieve the current application profile (mode, bind address, etc.).
    async fn get_app_profile(&self) -> Result<AppProfile>;
    /// Issue a new setup token that expires after the provided duration.
    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken>;
    /// Validate that a setup token remains active without consuming it.
    async fn validate_setup_token(&self, token: &str) -> Result<()>;
    /// Consume a setup token, preventing subsequent reuse.
    async fn consume_setup_token(&self, token: &str) -> Result<()>;
    /// Apply a configuration changeset attributed to the supplied actor.
    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges>;
    /// Obtain a strongly typed snapshot of the current configuration.
    async fn snapshot(&self) -> Result<ConfigSnapshot>;
    /// Validate API credentials and return the associated authorisation scope.
    async fn authenticate_api_key(&self, key_id: &str, secret: &str) -> Result<Option<ApiKeyAuth>>;
}

/// Shared reference to the configuration backend.
pub type SharedConfig = Arc<dyn ConfigFacade>;

#[async_trait]
impl ConfigFacade for ConfigService {
    async fn get_app_profile(&self) -> Result<AppProfile> {
        <Self as SettingsFacade>::get_app_profile(self).await
    }

    async fn issue_setup_token(&self, ttl: Duration, issued_by: &str) -> Result<SetupToken> {
        <Self as SettingsFacade>::issue_setup_token(self, ttl, issued_by).await
    }

    async fn validate_setup_token(&self, token: &str) -> Result<()> {
        Self::validate_setup_token(self, token).await
    }

    async fn consume_setup_token(&self, token: &str) -> Result<()> {
        <Self as SettingsFacade>::consume_setup_token(self, token).await
    }

    async fn apply_changeset(
        &self,
        actor: &str,
        reason: &str,
        changeset: SettingsChangeset,
    ) -> Result<AppliedChanges> {
        <Self as SettingsFacade>::apply_changeset(self, actor, reason, changeset).await
    }

    async fn snapshot(&self) -> Result<ConfigSnapshot> {
        Self::snapshot(self).await
    }

    async fn authenticate_api_key(&self, key_id: &str, secret: &str) -> Result<Option<ApiKeyAuth>> {
        Self::authenticate_api_key(self, key_id, secret).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use chrono::Utc;
    use std::collections::VecDeque;
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct RecordingConfig {
        calls: Arc<Mutex<VecDeque<&'static str>>>,
        profile: AppProfile,
        snapshot: ConfigSnapshot,
    }

    impl RecordingConfig {
        fn new(profile: AppProfile, snapshot: ConfigSnapshot) -> Self {
            Self {
                calls: Arc::default(),
                profile,
                snapshot,
            }
        }

        async fn pop_calls(&self) -> Vec<&'static str> {
            let mut guard = self.calls.lock().await;
            guard.drain(..).collect()
        }
    }

    #[async_trait]
    impl ConfigFacade for RecordingConfig {
        async fn get_app_profile(&self) -> Result<AppProfile> {
            self.calls.lock().await.push_back("get_app_profile");
            Ok(self.profile.clone())
        }

        async fn issue_setup_token(&self, _ttl: Duration, _issued_by: &str) -> Result<SetupToken> {
            self.calls.lock().await.push_back("issue_setup_token");
            Ok(SetupToken {
                plaintext: "token".into(),
                expires_at: Utc::now(),
            })
        }

        async fn validate_setup_token(&self, _token: &str) -> Result<()> {
            self.calls.lock().await.push_back("validate_setup_token");
            Ok(())
        }

        async fn consume_setup_token(&self, _token: &str) -> Result<()> {
            self.calls.lock().await.push_back("consume_setup_token");
            Ok(())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> Result<AppliedChanges> {
            self.calls.lock().await.push_back("apply_changeset");
            Err(anyhow!("not implemented"))
        }

        async fn snapshot(&self) -> Result<ConfigSnapshot> {
            self.calls.lock().await.push_back("snapshot");
            Ok(self.snapshot.clone())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> Result<Option<ApiKeyAuth>> {
            self.calls.lock().await.push_back("authenticate_api_key");
            Ok(None)
        }
    }

    #[tokio::test]
    async fn shared_config_trait_invokes_expected_methods() {
        let profile = AppProfile {
            id: uuid::Uuid::nil(),
            instance_name: "test".into(),
            mode: revaer_config::AppMode::Active,
            version: 1,
            http_port: 3000,
            bind_addr: "127.0.0.1".parse().expect("bind addr"),
            telemetry: serde_json::json!({}),
            features: serde_json::json!([]),
            immutable_keys: serde_json::json!([]),
        };
        let snapshot = ConfigSnapshot {
            revision: 1,
            app_profile: profile.clone(),
            engine_profile: revaer_config::EngineProfile {
                id: uuid::Uuid::nil(),
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
                tracker: serde_json::json!([]),
            },
            fs_policy: revaer_config::FsPolicy {
                id: uuid::Uuid::nil(),
                library_root: "/tmp/library".into(),
                extract: false,
                par2: "disabled".into(),
                flatten: false,
                move_mode: "copy".into(),
                cleanup_keep: serde_json::json!([]),
                cleanup_drop: serde_json::json!([]),
                chmod_file: None,
                chmod_dir: None,
                owner: None,
                group: None,
                umask: None,
                allow_paths: serde_json::json!([]),
            },
        };
        let config = RecordingConfig::new(profile.clone(), snapshot.clone());
        let shared: SharedConfig = Arc::new(config.clone());

        let _ = shared.get_app_profile().await.expect("profile");
        let _ = shared.snapshot().await.expect("snapshot");
        let _ = shared
            .issue_setup_token(Duration::from_secs(30), "tester")
            .await;
        let _ = shared.validate_setup_token("token").await;
        let _ = shared.consume_setup_token("token").await;
        let _ = shared
            .authenticate_api_key("id", "secret")
            .await
            .expect("auth result");

        let calls = config.pop_calls().await;
        assert_eq!(
            calls,
            [
                "get_app_profile",
                "snapshot",
                "issue_setup_token",
                "validate_setup_token",
                "consume_setup_token",
                "authenticate_api_key"
            ]
        );
    }
}
