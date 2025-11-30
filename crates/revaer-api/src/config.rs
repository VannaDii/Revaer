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
