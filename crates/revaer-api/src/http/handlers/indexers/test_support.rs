//! Shared test helpers for indexer handler modules.

use crate::app::indexers::{
    CategoryMappingServiceError, CategoryMappingServiceErrorKind,
    HealthNotificationHookUpdateParams, HealthNotificationServiceError,
    HealthNotificationServiceErrorKind, IndexerBackupServiceError, IndexerBackupServiceErrorKind,
    IndexerCfStateResetParams, IndexerDefinitionServiceError, IndexerFacade,
    IndexerHealthEventListParams, IndexerInstanceFieldError, IndexerInstanceFieldValueParams,
    IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
    IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams, IndexerRssSeenListParams,
    IndexerRssSeenMarkParams, IndexerRssSubscriptionParams, IndexerSourceReputationListParams,
    RateLimitPolicyServiceError, RateLimitPolicyServiceErrorKind, RoutingPolicyServiceError,
    RoutingPolicyServiceErrorKind, SearchProfileServiceError, SearchProfileServiceErrorKind,
    SearchRequestCreateParams, SearchRequestServiceError, SearchRequestServiceErrorKind,
    SecretServiceError, TagServiceError, TorznabInstanceCredentials, TorznabInstanceServiceError,
    TorznabInstanceServiceErrorKind,
};
use crate::app::state::ApiState;
use crate::config::ConfigFacade;
use crate::http::errors::ApiError;
use crate::models::{
    IndexerBackupExportResponse, IndexerBackupRestoreResponse, IndexerBackupSnapshot,
    IndexerCfStateResponse, IndexerConnectivityProfileResponse, IndexerDefinitionResponse,
    IndexerHealthEventResponse, IndexerHealthNotificationHookResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemResponse, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerSourceReputationResponse, ProblemDetails, SearchPageListResponse, SearchPageResponse,
    SearchRequestCreateResponse, SearchRequestExplainabilityResponse,
};
use async_trait::async_trait;
use axum::response::Response;
use revaer_config::{
    ApiKeyAuth, AppAuthMode, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult,
    ConfigSnapshot, SettingsChangeset, SetupToken, TelemetryConfig,
    validate::default_local_networks,
};
use revaer_events::EventBus;
use revaer_telemetry::Metrics;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const MAX_TEST_BODY_SIZE: usize = 1024 * 1024;
const MAX_TEST_BODY_PREVIEW_CHARS: usize = 200;
const TEST_APP_PROFILE_PUBLIC_ID: Uuid = Uuid::nil();
const DEFAULT_TAG_PUBLIC_ID: Uuid = Uuid::from_u128(1);
const DEFAULT_HEALTH_NOTIFICATION_HOOK_PUBLIC_ID: Uuid = Uuid::from_u128(2);
const DEFAULT_SEARCH_REQUEST_PUBLIC_ID: Uuid = Uuid::from_u128(3);
const DEFAULT_REQUEST_POLICY_SET_PUBLIC_ID: Uuid = Uuid::from_u128(4);
const DEFAULT_SECRET_PUBLIC_ID: Uuid = Uuid::from_u128(5);
const DEFAULT_INDEXER_INSTANCE_PUBLIC_ID: Uuid = Uuid::from_u128(6);

#[derive(Clone)]
struct StubConfig;

#[async_trait]
impl ConfigFacade for StubConfig {
    async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
        Ok(AppProfile {
            id: TEST_APP_PROFILE_PUBLIC_ID,
            instance_name: "test".into(),
            mode: AppMode::Active,
            auth_mode: AppAuthMode::ApiKey,
            version: 1,
            http_port: 8080,
            bind_addr: "127.0.0.1"
                .parse()
                .map_err(|_| ConfigError::InvalidBindAddr {
                    value: "127.0.0.1".to_string(),
                })?,
            local_networks: default_local_networks(),
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
        _: SettingsChangeset,
    ) -> ConfigResult<AppliedChanges> {
        Err(ConfigError::InvalidField {
            section: "config".to_string(),
            field: "changeset".to_string(),
            value: None,
            reason: "not implemented",
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
        Ok(false)
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

/// Test stub that captures secret values for assertions.
///
/// Production implementations must never log or persist secret material in
/// plain text; this helper is for test-only validation.
#[cfg(test)]
#[derive(Clone, Default)]
pub(super) struct RecordingIndexers {
    pub(super) created: Arc<Mutex<Vec<(String, String)>>>,
    pub(super) rotated: Arc<Mutex<Vec<(Uuid, String)>>>,
    pub(super) revoked: Arc<Mutex<Vec<Uuid>>>,
    pub(super) search_request_calls: Arc<Mutex<Vec<SearchRequestCreateSnapshot>>>,
    pub(super) search_request_create_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) search_request_cancel_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) search_page_list_response: Arc<Mutex<Option<SearchPageListResponse>>>,
    pub(super) search_page_fetch_response: Arc<Mutex<Option<SearchPageResponse>>>,
    pub(super) search_page_list_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) search_page_fetch_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) rss_subscription_response: Arc<Mutex<Option<IndexerRssSubscriptionResponse>>>,
    pub(super) rss_subscription_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) rss_seen_items_response: Arc<Mutex<Option<Vec<IndexerRssSeenItemResponse>>>>,
    pub(super) rss_seen_items_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) rss_seen_mark_response: Arc<Mutex<Option<IndexerRssSeenMarkResponse>>>,
    pub(super) rss_seen_mark_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) connectivity_profile_response:
        Arc<Mutex<Option<IndexerConnectivityProfileResponse>>>,
    pub(super) connectivity_profile_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) source_reputation_response: Arc<Mutex<Option<Vec<IndexerSourceReputationResponse>>>>,
    pub(super) source_reputation_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) health_event_response: Arc<Mutex<Option<Vec<IndexerHealthEventResponse>>>>,
    pub(super) health_event_error: Arc<Mutex<Option<IndexerInstanceServiceError>>>,
    pub(super) backup_export_response: Arc<Mutex<Option<IndexerBackupExportResponse>>>,
    pub(super) backup_export_error: Arc<Mutex<Option<IndexerBackupServiceError>>>,
    pub(super) backup_restore_response: Arc<Mutex<Option<IndexerBackupRestoreResponse>>>,
    pub(super) backup_restore_error: Arc<Mutex<Option<IndexerBackupServiceError>>>,
    pub(super) health_notification_hooks: Arc<Mutex<Vec<IndexerHealthNotificationHookResponse>>>,
    pub(super) health_notification_error: Arc<Mutex<Option<HealthNotificationServiceError>>>,
    pub(super) secret_error: Arc<Mutex<Option<SecretServiceError>>>,
    pub(super) tag_calls: Arc<Mutex<Vec<(Uuid, String, String)>>>,
    pub(super) tag_result: Arc<Mutex<Option<Result<Uuid, TagServiceError>>>>,
    pub(super) tag_error: Arc<Mutex<Option<TagServiceError>>>,
}

/// Snapshot of search request create inputs for assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SearchRequestCreateSnapshot {
    /// Optional actor identifier passed through the handler.
    pub(super) actor_user_public_id: Option<Uuid>,
    /// Trimmed query text.
    pub(super) query_text: String,
    /// Trimmed query type.
    pub(super) query_type: String,
    /// Optional torznab mode.
    pub(super) torznab_mode: Option<String>,
    /// Optional requested media domain key.
    pub(super) requested_media_domain_key: Option<String>,
}

#[cfg(test)]
#[async_trait]
impl IndexerFacade for RecordingIndexers {
    async fn indexer_definition_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
        Ok(Vec::new())
    }

    async fn indexer_health_notification_hook_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerHealthNotificationHookResponse>, HealthNotificationServiceError> {
        let error = self
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(err) = error {
            return Err(err);
        }
        Ok(self
            .health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn indexer_health_notification_hook_create(
        &self,
        _actor_user_public_id: Uuid,
        channel: &str,
        display_name: &str,
        status_threshold: &str,
        webhook_url: Option<&str>,
        email: Option<&str>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        let error = self
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(err) = error {
            return Err(err);
        }
        let hook_public_id = DEFAULT_HEALTH_NOTIFICATION_HOOK_PUBLIC_ID;
        self.health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(IndexerHealthNotificationHookResponse {
                indexer_health_notification_hook_public_id: hook_public_id,
                channel: channel.to_string(),
                display_name: display_name.to_string(),
                status_threshold: status_threshold.to_string(),
                webhook_url: webhook_url.map(str::to_string),
                email: email.map(str::to_string),
                is_enabled: true,
                updated_at: chrono::Utc::now(),
            });
        Ok(hook_public_id)
    }

    async fn indexer_health_notification_hook_get(
        &self,
        _actor_user_public_id: Uuid,
        hook_public_id: Uuid,
    ) -> Result<IndexerHealthNotificationHookResponse, HealthNotificationServiceError> {
        let error = self
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(err) = error {
            return Err(err);
        }

        self.health_notification_hooks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|hook| hook.indexer_health_notification_hook_public_id == hook_public_id)
            .cloned()
            .ok_or_else(|| {
                HealthNotificationServiceError::new(HealthNotificationServiceErrorKind::NotFound)
                    .with_code("hook_not_found")
            })
    }

    async fn indexer_health_notification_hook_update(
        &self,
        params: HealthNotificationHookUpdateParams<'_>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        let error = self
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(err) = error {
            return Err(err);
        }
        {
            let mut hooks = self
                .health_notification_hooks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(hook) = hooks.iter_mut().find(|item| {
                item.indexer_health_notification_hook_public_id == params.hook_public_id
            }) else {
                return Err(HealthNotificationServiceError::new(
                    HealthNotificationServiceErrorKind::NotFound,
                )
                .with_code("hook_not_found"));
            };
            if let Some(value) = params.display_name {
                hook.display_name = value.to_string();
            }
            if let Some(value) = params.status_threshold {
                hook.status_threshold = value.to_string();
            }
            if let Some(value) = params.webhook_url {
                hook.webhook_url = Some(value.to_string());
            }
            if let Some(value) = params.email {
                hook.email = Some(value.to_string());
            }
            if let Some(value) = params.is_enabled {
                hook.is_enabled = value;
            }
            hook.updated_at = chrono::Utc::now();
            drop(hooks);
        }
        Ok(params.hook_public_id)
    }

    async fn indexer_health_notification_hook_delete(
        &self,
        _actor_user_public_id: Uuid,
        hook_public_id: Uuid,
    ) -> Result<(), HealthNotificationServiceError> {
        let error = self
            .health_notification_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(err) = error {
            return Err(err);
        }
        {
            let mut hooks = self
                .health_notification_hooks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let len_before = hooks.len();
            hooks.retain(|item| item.indexer_health_notification_hook_public_id != hook_public_id);
            if hooks.len() == len_before {
                return Err(HealthNotificationServiceError::new(
                    HealthNotificationServiceErrorKind::NotFound,
                )
                .with_code("hook_not_found"));
            }
        }
        Ok(())
    }

    async fn search_profile_create(
        &self,
        _actor_user_public_id: Uuid,
        _display_name: &str,
        _is_default: Option<bool>,
        _page_size: Option<i32>,
        _default_media_domain_key: Option<&str>,
        _user_public_id: Option<Uuid>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_update(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _display_name: Option<&str>,
        _page_size: Option<i32>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_set_default(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _page_size: Option<i32>,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_set_default_domain(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _default_media_domain_key: Option<&str>,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_set_domain_allowlist(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _media_domain_keys: &[String],
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_add_policy_set(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_remove_policy_set(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_indexer_allow(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_indexer_block(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_tag_allow(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _tag_public_ids: Option<&[Uuid]>,
        _tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_tag_block(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _tag_public_ids: Option<&[Uuid]>,
        _tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_profile_tag_prefer(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _tag_public_ids: Option<&[Uuid]>,
        _tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        Err(SearchProfileServiceError::new(
            SearchProfileServiceErrorKind::Storage,
        ))
    }

    async fn search_request_create(
        &self,
        params: SearchRequestCreateParams<'_>,
    ) -> Result<SearchRequestCreateResponse, SearchRequestServiceError> {
        let create_error = self
            .search_request_create_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = create_error {
            return Err(error);
        }

        let snapshot = SearchRequestCreateSnapshot {
            actor_user_public_id: params.actor_user_public_id,
            query_text: params.query_text.to_string(),
            query_type: params.query_type.to_string(),
            torznab_mode: params.torznab_mode.map(str::to_string),
            requested_media_domain_key: params.requested_media_domain_key.map(str::to_string),
        };
        self.search_request_calls
            .lock()
            .expect("lock")
            .push(snapshot);

        Ok(SearchRequestCreateResponse {
            search_request_public_id: DEFAULT_SEARCH_REQUEST_PUBLIC_ID,
            request_policy_set_public_id: DEFAULT_REQUEST_POLICY_SET_PUBLIC_ID,
        })
    }

    async fn search_request_cancel(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
    ) -> Result<(), SearchRequestServiceError> {
        let cancel_error = self
            .search_request_cancel_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = cancel_error {
            return Err(error);
        }
        Ok(())
    }

    async fn search_page_list(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
    ) -> Result<SearchPageListResponse, SearchRequestServiceError> {
        let list_error = self.search_page_list_error.lock().expect("lock").take();
        if let Some(error) = list_error {
            return Err(error);
        }

        let response = self.search_page_list_response.lock().expect("lock").take();
        Ok(response.unwrap_or(SearchPageListResponse {
            pages: Vec::new(),
            explainability: SearchRequestExplainabilityResponse {
                zero_runnable_indexers: true,
                skipped_canceled_indexers: 0,
                skipped_failed_indexers: 0,
                blocked_results: 0,
                blocked_rule_public_ids: Vec::new(),
                rate_limited_indexers: 0,
                retrying_indexers: 0,
            },
        }))
    }

    async fn search_page_fetch(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
        _page_number: i32,
    ) -> Result<SearchPageResponse, SearchRequestServiceError> {
        let fetch_error = self.search_page_fetch_error.lock().expect("lock").take();
        if let Some(error) = fetch_error {
            return Err(error);
        }

        let response = self.search_page_fetch_response.lock().expect("lock").take();
        response
            .ok_or_else(|| SearchRequestServiceError::new(SearchRequestServiceErrorKind::Storage))
    }

    async fn tag_create(
        &self,
        actor_user_public_id: Uuid,
        tag_key: &str,
        display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        self.tag_calls.lock().expect("lock poisoned").push((
            actor_user_public_id,
            tag_key.to_string(),
            display_name.to_string(),
        ));
        let tag_result = self.tag_result.lock().expect("lock poisoned").take();
        if let Some(result) = tag_result {
            return result;
        }
        let tag_error = self.tag_error.lock().expect("lock poisoned").take();
        if let Some(error) = tag_error {
            return Err(error);
        }
        Ok(DEFAULT_TAG_PUBLIC_ID)
    }

    async fn tag_update(
        &self,
        _actor_user_public_id: Uuid,
        tag_public_id: Option<Uuid>,
        _tag_key: Option<&str>,
        _display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        let tag_error = self.tag_error.lock().expect("lock poisoned").take();
        if let Some(error) = tag_error {
            return Err(error);
        }
        Ok(tag_public_id.unwrap_or(DEFAULT_TAG_PUBLIC_ID))
    }

    async fn tag_delete(
        &self,
        _actor_user_public_id: Uuid,
        _tag_public_id: Option<Uuid>,
        _tag_key: Option<&str>,
    ) -> Result<(), TagServiceError> {
        let tag_error = self.tag_error.lock().expect("lock poisoned").take();
        if let Some(error) = tag_error {
            return Err(error);
        }
        Ok(())
    }

    async fn indexer_backup_export(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<IndexerBackupExportResponse, IndexerBackupServiceError> {
        let error = self.backup_export_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.backup_export_response.lock().expect("lock").take();
        response
            .ok_or_else(|| IndexerBackupServiceError::new(IndexerBackupServiceErrorKind::Storage))
    }

    async fn indexer_backup_restore(
        &self,
        _actor_user_public_id: Uuid,
        _snapshot: &IndexerBackupSnapshot,
    ) -> Result<IndexerBackupRestoreResponse, IndexerBackupServiceError> {
        let error = self.backup_restore_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.backup_restore_response.lock().expect("lock").take();
        response
            .ok_or_else(|| IndexerBackupServiceError::new(IndexerBackupServiceErrorKind::Storage))
    }

    async fn routing_policy_create(
        &self,
        _actor_user_public_id: Uuid,
        _display_name: &str,
        _mode: &str,
    ) -> Result<Uuid, RoutingPolicyServiceError> {
        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }

    async fn routing_policy_set_param(
        &self,
        _actor_user_public_id: Uuid,
        _routing_policy_public_id: Uuid,
        _param_key: &str,
        _value_plain: Option<&str>,
        _value_int: Option<i32>,
        _value_bool: Option<bool>,
    ) -> Result<(), RoutingPolicyServiceError> {
        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }

    async fn routing_policy_bind_secret(
        &self,
        _actor_user_public_id: Uuid,
        _routing_policy_public_id: Uuid,
        _param_key: &str,
        _secret_public_id: Uuid,
    ) -> Result<(), RoutingPolicyServiceError> {
        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }

    async fn rate_limit_policy_create(
        &self,
        _actor_user_public_id: Uuid,
        _display_name: &str,
        _rpm: i32,
        _burst: i32,
        _concurrent: i32,
    ) -> Result<Uuid, RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }

    async fn rate_limit_policy_update(
        &self,
        _actor_user_public_id: Uuid,
        _rate_limit_policy_public_id: Uuid,
        _display_name: Option<&str>,
        _rpm: Option<i32>,
        _burst: Option<i32>,
        _concurrent: Option<i32>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }

    async fn rate_limit_policy_soft_delete(
        &self,
        _actor_user_public_id: Uuid,
        _rate_limit_policy_public_id: Uuid,
    ) -> Result<(), RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_set_rate_limit_policy(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }

    async fn routing_policy_set_rate_limit_policy(
        &self,
        _actor_user_public_id: Uuid,
        _routing_policy_public_id: Uuid,
        _rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }

    async fn tracker_category_mapping_upsert(
        &self,
        _params: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        Err(CategoryMappingServiceError::new(
            CategoryMappingServiceErrorKind::Storage,
        ))
    }

    async fn tracker_category_mapping_delete(
        &self,
        _params: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        Err(CategoryMappingServiceError::new(
            CategoryMappingServiceErrorKind::Storage,
        ))
    }

    async fn media_domain_mapping_upsert(
        &self,
        _actor_user_public_id: Uuid,
        _media_domain_key: &str,
        _torznab_cat_id: i32,
        _is_primary: Option<bool>,
    ) -> Result<(), CategoryMappingServiceError> {
        Err(CategoryMappingServiceError::new(
            CategoryMappingServiceErrorKind::Storage,
        ))
    }

    async fn media_domain_mapping_delete(
        &self,
        _actor_user_public_id: Uuid,
        _media_domain_key: &str,
        _torznab_cat_id: i32,
    ) -> Result<(), CategoryMappingServiceError> {
        Err(CategoryMappingServiceError::new(
            CategoryMappingServiceErrorKind::Storage,
        ))
    }

    async fn torznab_instance_create(
        &self,
        _actor_user_public_id: Uuid,
        _search_profile_public_id: Uuid,
        _display_name: &str,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        Err(TorznabInstanceServiceError::new(
            TorznabInstanceServiceErrorKind::Storage,
        ))
    }

    async fn torznab_instance_rotate_key(
        &self,
        _actor_user_public_id: Uuid,
        _torznab_instance_public_id: Uuid,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        Err(TorznabInstanceServiceError::new(
            TorznabInstanceServiceErrorKind::Storage,
        ))
    }

    async fn torznab_instance_enable_disable(
        &self,
        _actor_user_public_id: Uuid,
        _torznab_instance_public_id: Uuid,
        _is_enabled: bool,
    ) -> Result<(), TorznabInstanceServiceError> {
        Err(TorznabInstanceServiceError::new(
            TorznabInstanceServiceErrorKind::Storage,
        ))
    }

    async fn torznab_instance_soft_delete(
        &self,
        _actor_user_public_id: Uuid,
        _torznab_instance_public_id: Uuid,
    ) -> Result<(), TorznabInstanceServiceError> {
        Err(TorznabInstanceServiceError::new(
            TorznabInstanceServiceErrorKind::Storage,
        ))
    }

    async fn secret_create(
        &self,
        _actor_user_public_id: Uuid,
        secret_type: &str,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        let secret_error = self.secret_error.lock().expect("lock poisoned").take();
        if let Some(error) = secret_error {
            return Err(error);
        }
        self.created
            .lock()
            .expect("lock poisoned")
            .push((secret_type.to_string(), secret_value.to_string()));
        Ok(DEFAULT_SECRET_PUBLIC_ID)
    }

    async fn secret_rotate(
        &self,
        _actor_user_public_id: Uuid,
        secret_public_id: Uuid,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        let secret_error = self.secret_error.lock().expect("lock poisoned").take();
        if let Some(error) = secret_error {
            return Err(error);
        }
        self.rotated
            .lock()
            .expect("lock poisoned")
            .push((secret_public_id, secret_value.to_string()));
        Ok(secret_public_id)
    }

    async fn secret_revoke(
        &self,
        _actor_user_public_id: Uuid,
        secret_public_id: Uuid,
    ) -> Result<(), SecretServiceError> {
        let secret_error = self.secret_error.lock().expect("lock poisoned").take();
        if let Some(error) = secret_error {
            return Err(error);
        }
        self.revoked
            .lock()
            .expect("lock poisoned")
            .push(secret_public_id);
        Ok(())
    }

    async fn indexer_instance_create(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_definition_upstream_slug: &str,
        _display_name: &str,
        _priority: Option<i32>,
        _trust_tier_key: Option<&str>,
        _routing_policy_public_id: Option<Uuid>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        Ok(DEFAULT_INDEXER_INSTANCE_PUBLIC_ID)
    }

    async fn indexer_instance_update(
        &self,
        _params: IndexerInstanceUpdateParams<'_>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        Ok(_params.indexer_instance_public_id)
    }

    async fn indexer_instance_set_media_domains(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _media_domain_keys: &[String],
    ) -> Result<(), IndexerInstanceServiceError> {
        Ok(())
    }

    async fn indexer_instance_set_tags(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _tag_public_ids: Option<&[Uuid]>,
        _tag_keys: Option<&[String]>,
    ) -> Result<(), IndexerInstanceServiceError> {
        Ok(())
    }

    async fn indexer_instance_field_set_value(
        &self,
        _params: IndexerInstanceFieldValueParams<'_>,
    ) -> Result<(), IndexerInstanceFieldError> {
        Ok(())
    }

    async fn indexer_instance_field_bind_secret(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _field_name: &str,
        _secret_public_id: Uuid,
    ) -> Result<(), IndexerInstanceFieldError> {
        Ok(())
    }

    async fn indexer_cf_state_reset(
        &self,
        _params: IndexerCfStateResetParams<'_>,
    ) -> Result<(), IndexerInstanceServiceError> {
        Ok(())
    }

    async fn indexer_cf_state_get(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
    ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_connectivity_profile_get(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
    ) -> Result<IndexerConnectivityProfileResponse, IndexerInstanceServiceError> {
        let error = self.connectivity_profile_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self
            .connectivity_profile_response
            .lock()
            .expect("lock")
            .take();
        Ok(response.unwrap_or(IndexerConnectivityProfileResponse {
            profile_exists: false,
            status: None,
            error_class: None,
            latency_p50_ms: None,
            latency_p95_ms: None,
            success_rate_1h: None,
            success_rate_24h: None,
            last_checked_at: None,
        }))
    }

    async fn indexer_source_reputation_list(
        &self,
        _params: IndexerSourceReputationListParams<'_>,
    ) -> Result<Vec<IndexerSourceReputationResponse>, IndexerInstanceServiceError> {
        let error = self.source_reputation_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.source_reputation_response.lock().expect("lock").take();
        Ok(response.unwrap_or_default())
    }

    async fn indexer_health_event_list(
        &self,
        _params: IndexerHealthEventListParams,
    ) -> Result<Vec<IndexerHealthEventResponse>, IndexerInstanceServiceError> {
        let error = self.health_event_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.health_event_response.lock().expect("lock").take();
        Ok(response.unwrap_or_default())
    }

    async fn indexer_instance_test_prepare(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
    ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_test_finalize(
        &self,
        _params: IndexerInstanceTestFinalizeParams<'_>,
    ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_rss_subscription_get(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let response = self.rss_subscription_response.lock().expect("lock").take();
        response.ok_or_else(|| {
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Storage)
        })
    }

    async fn indexer_rss_subscription_set(
        &self,
        _params: IndexerRssSubscriptionParams,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let error = self.rss_subscription_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.rss_subscription_response.lock().expect("lock").take();
        response.ok_or_else(|| {
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Storage)
        })
    }

    async fn indexer_rss_seen_list(
        &self,
        _params: IndexerRssSeenListParams,
    ) -> Result<Vec<IndexerRssSeenItemResponse>, IndexerInstanceServiceError> {
        let error = self.rss_seen_items_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.rss_seen_items_response.lock().expect("lock").take();
        Ok(response.unwrap_or_default())
    }

    async fn indexer_rss_seen_mark(
        &self,
        _params: IndexerRssSeenMarkParams<'_>,
    ) -> Result<IndexerRssSeenMarkResponse, IndexerInstanceServiceError> {
        let error = self.rss_seen_mark_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.rss_seen_mark_response.lock().expect("lock").take();
        response.ok_or_else(|| {
            IndexerInstanceServiceError::new(IndexerInstanceServiceErrorKind::Storage)
        })
    }
}

pub(super) fn indexer_test_state(
    indexers: Arc<dyn IndexerFacade>,
) -> Result<Arc<ApiState>, ApiError> {
    let telemetry = Metrics::new().map_err(|_| ApiError::internal("metrics init failed"))?;
    Ok(Arc::new(ApiState::new(
        Arc::new(StubConfig),
        indexers,
        telemetry,
        Arc::new(json!({})),
        EventBus::with_capacity(4),
        None,
    )))
}

pub(super) async fn parse_problem(response: Response) -> ProblemDetails {
    let body = axum::body::to_bytes(response.into_body(), MAX_TEST_BODY_SIZE)
        .await
        .expect("failed to read response body for ProblemDetails");
    let body_text = String::from_utf8_lossy(&body);
    match serde_json::from_slice(&body) {
        Ok(problem) => problem,
        Err(error) => {
            let body_char_count = body_text.chars().count();
            let body_preview: String = body_text
                .chars()
                .take(MAX_TEST_BODY_PREVIEW_CHARS)
                .collect();
            let truncated_suffix = if body_char_count > MAX_TEST_BODY_PREVIEW_CHARS {
                " [truncated]"
            } else {
                ""
            };
            panic!(
                "failed to deserialize ProblemDetails from response body (preview, max {MAX_TEST_BODY_PREVIEW_CHARS} chars): {body_preview}{truncated_suffix} ({error})"
            )
        }
    }
}
