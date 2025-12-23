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
    use revaer_config::normalize_engine_profile;
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

    fn sample_app_profile() -> AppProfile {
        AppProfile {
            id: uuid::Uuid::nil(),
            instance_name: "test".into(),
            mode: revaer_config::AppMode::Active,
            version: 1,
            http_port: 3000,
            bind_addr: "127.0.0.1".parse().expect("bind addr"),
            telemetry: serde_json::json!({}),
            features: serde_json::json!({}),
            immutable_keys: serde_json::json!([]),
        }
    }

    fn sample_engine_profile() -> revaer_config::EngineProfile {
        revaer_config::EngineProfile {
            id: uuid::Uuid::nil(),
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
            alt_speed: serde_json::json!({}),
            sequential_default: false,
            auto_managed: true.into(),
            auto_manage_prefer_seeds: false.into(),
            dont_count_slow_torrents: true.into(),
            super_seeding: false.into(),
            choking_algorithm: revaer_config::EngineProfile::default_choking_algorithm(),
            seed_choking_algorithm: revaer_config::EngineProfile::default_seed_choking_algorithm(),
            strict_super_seeding: false.into(),
            optimistic_unchoke_slots: None,
            max_queued_disk_bytes: None,
            resume_dir: "/tmp".into(),
            download_root: "/tmp/downloads".into(),
            storage_mode: revaer_config::EngineProfile::default_storage_mode(),
            use_partfile: revaer_config::EngineProfile::default_use_partfile(),
            disk_read_mode: None,
            disk_write_mode: None,
            verify_piece_hashes: revaer_config::EngineProfile::default_verify_piece_hashes(),
            cache_size: None,
            cache_expiry: None,
            coalesce_reads: revaer_config::EngineProfile::default_coalesce_reads(),
            coalesce_writes: revaer_config::EngineProfile::default_coalesce_writes(),
            use_disk_cache_pool: revaer_config::EngineProfile::default_use_disk_cache_pool(),
            tracker: serde_json::json!([]),
            enable_lsd: false.into(),
            enable_upnp: false.into(),
            enable_natpmp: false.into(),
            enable_pex: false.into(),
            dht_bootstrap_nodes: Vec::new(),
            dht_router_nodes: Vec::new(),
            ip_filter: serde_json::json!({}),
            peer_classes: serde_json::json!({}),
            outgoing_port_min: None,
            outgoing_port_max: None,
            peer_dscp: None,
        }
    }

    fn sample_snapshot(
        profile: &AppProfile,
        engine_profile: &revaer_config::EngineProfile,
    ) -> ConfigSnapshot {
        ConfigSnapshot {
            revision: 1,
            app_profile: profile.clone(),
            engine_profile: engine_profile.clone(),
            engine_profile_effective: normalize_engine_profile(engine_profile),
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
        let profile = sample_app_profile();
        let engine_profile = sample_engine_profile();
        let snapshot = sample_snapshot(&profile, &engine_profile);
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
