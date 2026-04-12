//! Shared test helpers for indexer handler modules.

use crate::app::indexers::{
    CategoryMappingServiceError, HealthNotificationHookUpdateParams,
    HealthNotificationServiceError, HealthNotificationServiceErrorKind, ImportJobServiceError,
    IndexerBackupServiceError, IndexerBackupServiceErrorKind, IndexerCfStateResetParams,
    IndexerDefinitionServiceError, IndexerDefinitionServiceErrorKind, IndexerFacade,
    IndexerHealthEventListParams, IndexerInstanceFieldError, IndexerInstanceFieldValueParams,
    IndexerInstanceServiceError, IndexerInstanceServiceErrorKind,
    IndexerInstanceTestFinalizeParams, IndexerInstanceUpdateParams, IndexerRssSeenListParams,
    IndexerRssSeenMarkParams, IndexerRssSubscriptionParams, IndexerSourceReputationListParams,
    PolicyRuleCreateParams, PolicyServiceError, RateLimitPolicyServiceError,
    RoutingPolicyServiceError, RoutingPolicyServiceErrorKind, SearchProfileServiceError,
    SearchRequestCreateParams, SearchRequestServiceError, SearchRequestServiceErrorKind,
    SecretServiceError, SourceMetadataConflictServiceError, TagServiceError, TorznabAccessError,
    TorznabAccessErrorKind, TorznabCategory, TorznabInstanceAuth, TorznabInstanceCredentials,
    TorznabInstanceServiceError,
};
use crate::app::state::ApiState;
use crate::config::ConfigFacade;
use crate::http::errors::ApiError;
use crate::models::{
    CardigannDefinitionImportResponse, ImportJobResultResponse, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerBackupRestoreResponse, IndexerBackupSnapshot,
    IndexerCfStateResponse, IndexerConnectivityProfileResponse, IndexerDefinitionResponse,
    IndexerHealthEventResponse, IndexerHealthNotificationHookResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemResponse, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerSourceMetadataConflictResponse, IndexerSourceReputationResponse,
    PolicySetListItemResponse, ProblemDetails, RateLimitPolicyListItemResponse,
    RoutingPolicyDetailResponse, RoutingPolicyListItemResponse, SearchPageListResponse,
    SearchPageResponse, SearchProfileListItemResponse, SearchRequestCreateResponse,
    SearchRequestExplainabilityResponse, SecretMetadataResponse, TagListItemResponse,
    TorznabInstanceListItemResponse,
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
const DEFAULT_IMPORT_JOB_PUBLIC_ID: Uuid = Uuid::from_u128(7);
const DEFAULT_RATE_LIMIT_POLICY_PUBLIC_ID: Uuid = Uuid::from_u128(8);
const DEFAULT_TORZNAB_INSTANCE_PUBLIC_ID: Uuid = Uuid::from_u128(9);
const DEFAULT_ROUTING_POLICY_PUBLIC_ID: Uuid = Uuid::from_u128(10);
const DEFAULT_SEARCH_PROFILE_PUBLIC_ID: Uuid = Uuid::from_u128(11);

type SourceMetadataConflictResolveCall = (Uuid, i64, String, Option<String>);
type SourceMetadataConflictReopenCall = (Uuid, i64, Option<String>);
type TrackerCategoryMappingUpsertCall = (
    Option<Uuid>,
    Option<String>,
    Option<Uuid>,
    i32,
    Option<i32>,
    i32,
    Option<String>,
);
type TrackerCategoryMappingDeleteCall =
    (Option<Uuid>, Option<String>, Option<Uuid>, i32, Option<i32>);
type MediaDomainMappingUpsertCall = (Uuid, String, i32, Option<bool>);
type MediaDomainMappingDeleteCall = (Uuid, String, i32);
type ImportJobCreateCall = (Uuid, String, Option<bool>, Option<Uuid>, Option<Uuid>);
type ImportJobRunProwlarrApiCall = (Uuid, String, Uuid);
type ImportJobRunProwlarrBackupCall = (Uuid, String);
type DefinitionImportCall = (Uuid, String, Option<bool>);
type RateLimitCreateCall = (String, i32, i32, i32);
type RateLimitUpdateCall = (Uuid, Option<String>, Option<i32>, Option<i32>, Option<i32>);
type RateLimitAssignmentCall = (Uuid, Option<Uuid>);
type TorznabCreateCall = (Uuid, Uuid, String);
type TorznabRotateCall = (Uuid, Uuid);
type TorznabStateCall = (Uuid, Uuid, bool);
type TorznabDeleteCall = (Uuid, Uuid);
type IndexerDefinitionListResult =
    Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError>;
type RoutingPolicyListResult =
    Result<Vec<RoutingPolicyListItemResponse>, RoutingPolicyServiceError>;
type TorznabAuthResult = Result<TorznabInstanceAuth, TorznabAccessError>;
type TorznabDownloadPrepareResult = Result<Option<String>, TorznabAccessError>;
type TorznabCategoryListResult = Result<Vec<TorznabCategory>, TorznabAccessError>;
type TorznabFeedCategoryResult = Result<Vec<i32>, TorznabAccessError>;
type RoutingPolicyCreateCall = (Uuid, String, String);
type RoutingPolicyGetCall = (Uuid, Uuid);
type RoutingPolicySetParamCall = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    Option<i32>,
    Option<bool>,
);
type RoutingPolicyBindSecretCall = (Uuid, Uuid, String, Uuid);
type SearchProfileCreateCall = (
    Uuid,
    String,
    Option<bool>,
    Option<i32>,
    Option<String>,
    Option<Uuid>,
);
type SearchProfileUpdateCall = (Uuid, Uuid, Option<String>, Option<i32>);
type SearchProfileDefaultCall = (Uuid, Uuid, Option<i32>);
type SearchProfileDefaultDomainCall = (Uuid, Uuid, Option<String>);
type SearchProfileDomainAllowlistCall = (Uuid, Uuid, Vec<String>);
type SearchProfilePolicySetCall = (Uuid, Uuid, Uuid);
type SearchProfileIndexerCall = (Uuid, Uuid, Vec<Uuid>);
type SearchProfileTagCall = (Uuid, Uuid, Option<Vec<Uuid>>, Option<Vec<String>>);
type PolicySetCreateCall = (Uuid, String, String, Option<bool>);
type PolicySetUpdateCall = (Uuid, Uuid, Option<String>);
type PolicySetToggleCall = (Uuid, Uuid);
type PolicySetReorderCall = (Uuid, Vec<Uuid>);
type PolicyRuleToggleCall = (Uuid, Uuid);
type PolicyRuleReorderCall = (Uuid, Uuid, Vec<Uuid>);
type TorznabAuthCall = (Uuid, String);
type TorznabDownloadPrepareCall = (Uuid, Uuid);
type TorznabFeedCategoryIdsCall = (Uuid, Uuid, Option<i32>, Option<i32>);

fn take_locked<T>(slot: &Mutex<Option<T>>) -> Option<T> {
    slot.lock().expect("lock").take()
}
type SourceMetadataConflictListCall = (Uuid, Option<bool>, Option<i32>);

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
pub(crate) struct RecordingIndexers {
    pub(super) created: Arc<Mutex<Vec<(String, String)>>>,
    pub(super) rotated: Arc<Mutex<Vec<(Uuid, String)>>>,
    pub(super) revoked: Arc<Mutex<Vec<Uuid>>>,
    pub(super) indexer_definition_list_calls: Arc<Mutex<Vec<Uuid>>>,
    pub(super) indexer_definition_import_calls: Arc<Mutex<Vec<DefinitionImportCall>>>,
    pub(super) indexer_definition_list_result: Arc<Mutex<Option<IndexerDefinitionListResult>>>,
    pub(super) indexer_definition_import_result: Arc<
        Mutex<Option<Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError>>>,
    >,
    pub(super) search_request_calls: Arc<Mutex<Vec<SearchRequestCreateSnapshot>>>,
    pub(super) search_request_create_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) search_request_cancel_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) import_job_create_calls: Arc<Mutex<Vec<ImportJobCreateCall>>>,
    pub(super) import_job_run_prowlarr_api_calls: Arc<Mutex<Vec<ImportJobRunProwlarrApiCall>>>,
    pub(super) import_job_run_prowlarr_backup_calls:
        Arc<Mutex<Vec<ImportJobRunProwlarrBackupCall>>>,
    pub(super) import_job_create_error: Arc<Mutex<Option<ImportJobServiceError>>>,
    pub(super) import_job_run_prowlarr_api_error: Arc<Mutex<Option<ImportJobServiceError>>>,
    pub(super) import_job_run_prowlarr_backup_error: Arc<Mutex<Option<ImportJobServiceError>>>,
    pub(super) import_job_status_response: Arc<Mutex<Option<ImportJobStatusResponse>>>,
    pub(super) import_job_status_error: Arc<Mutex<Option<ImportJobServiceError>>>,
    pub(super) import_job_results_response: Arc<Mutex<Option<Vec<ImportJobResultResponse>>>>,
    pub(super) import_job_results_error: Arc<Mutex<Option<ImportJobServiceError>>>,
    pub(super) rate_limit_create_calls: Arc<Mutex<Vec<RateLimitCreateCall>>>,
    pub(super) rate_limit_update_calls: Arc<Mutex<Vec<RateLimitUpdateCall>>>,
    pub(super) rate_limit_deleted_calls: Arc<Mutex<Vec<Uuid>>>,
    pub(super) indexer_rate_limit_assignment_calls: Arc<Mutex<Vec<RateLimitAssignmentCall>>>,
    pub(super) routing_rate_limit_assignment_calls: Arc<Mutex<Vec<RateLimitAssignmentCall>>>,
    pub(super) rate_limit_list_items: Arc<Mutex<Vec<RateLimitPolicyListItemResponse>>>,
    pub(super) rate_limit_create_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) rate_limit_update_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) rate_limit_delete_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) indexer_rate_limit_assignment_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) routing_rate_limit_assignment_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) rate_limit_list_error: Arc<Mutex<Option<RateLimitPolicyServiceError>>>,
    pub(super) torznab_instance_create_calls: Arc<Mutex<Vec<TorznabCreateCall>>>,
    pub(super) torznab_instance_rotate_calls: Arc<Mutex<Vec<TorznabRotateCall>>>,
    pub(super) torznab_instance_state_calls: Arc<Mutex<Vec<TorznabStateCall>>>,
    pub(super) torznab_instance_delete_calls: Arc<Mutex<Vec<TorznabDeleteCall>>>,
    pub(super) torznab_instance_list_items: Arc<Mutex<Vec<TorznabInstanceListItemResponse>>>,
    pub(super) torznab_instance_create_result:
        Arc<Mutex<Option<Result<TorznabInstanceCredentials, TorznabInstanceServiceError>>>>,
    pub(super) torznab_instance_rotate_result:
        Arc<Mutex<Option<Result<TorznabInstanceCredentials, TorznabInstanceServiceError>>>>,
    pub(super) torznab_instance_state_error: Arc<Mutex<Option<TorznabInstanceServiceError>>>,
    pub(super) torznab_instance_delete_error: Arc<Mutex<Option<TorznabInstanceServiceError>>>,
    pub(super) torznab_instance_list_error: Arc<Mutex<Option<TorznabInstanceServiceError>>>,
    pub(super) routing_policy_create_calls: Arc<Mutex<Vec<RoutingPolicyCreateCall>>>,
    pub(super) routing_policy_get_calls: Arc<Mutex<Vec<RoutingPolicyGetCall>>>,
    pub(super) routing_policy_set_param_calls: Arc<Mutex<Vec<RoutingPolicySetParamCall>>>,
    pub(super) routing_policy_bind_secret_calls: Arc<Mutex<Vec<RoutingPolicyBindSecretCall>>>,
    pub(super) routing_policy_create_result:
        Arc<Mutex<Option<Result<Uuid, RoutingPolicyServiceError>>>>,
    pub(super) routing_policy_get_result:
        Arc<Mutex<Option<Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError>>>>,
    pub(super) routing_policy_list_result: Arc<Mutex<Option<RoutingPolicyListResult>>>,
    pub(super) routing_policy_set_param_error: Arc<Mutex<Option<RoutingPolicyServiceError>>>,
    pub(super) routing_policy_bind_secret_error: Arc<Mutex<Option<RoutingPolicyServiceError>>>,
    pub(super) search_profile_create_calls: Arc<Mutex<Vec<SearchProfileCreateCall>>>,
    pub(super) search_profile_update_calls: Arc<Mutex<Vec<SearchProfileUpdateCall>>>,
    pub(super) search_profile_default_calls: Arc<Mutex<Vec<SearchProfileDefaultCall>>>,
    pub(super) search_profile_default_domain_calls: Arc<Mutex<Vec<SearchProfileDefaultDomainCall>>>,
    pub(super) search_profile_domain_allowlist_calls:
        Arc<Mutex<Vec<SearchProfileDomainAllowlistCall>>>,
    pub(super) search_profile_add_policy_set_calls: Arc<Mutex<Vec<SearchProfilePolicySetCall>>>,
    pub(super) search_profile_remove_policy_set_calls: Arc<Mutex<Vec<SearchProfilePolicySetCall>>>,
    pub(super) search_profile_indexer_allow_calls: Arc<Mutex<Vec<SearchProfileIndexerCall>>>,
    pub(super) search_profile_indexer_block_calls: Arc<Mutex<Vec<SearchProfileIndexerCall>>>,
    pub(super) search_profile_tag_allow_calls: Arc<Mutex<Vec<SearchProfileTagCall>>>,
    pub(super) search_profile_tag_block_calls: Arc<Mutex<Vec<SearchProfileTagCall>>>,
    pub(super) search_profile_tag_prefer_calls: Arc<Mutex<Vec<SearchProfileTagCall>>>,
    pub(super) search_profile_list_calls: Arc<Mutex<Vec<Uuid>>>,
    pub(super) search_profile_list_items: Arc<Mutex<Vec<SearchProfileListItemResponse>>>,
    pub(super) search_profile_create_result:
        Arc<Mutex<Option<Result<Uuid, SearchProfileServiceError>>>>,
    pub(super) search_profile_list_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_update_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_default_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_default_domain_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_domain_allowlist_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_add_policy_set_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_remove_policy_set_error:
        Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_indexer_allow_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_indexer_block_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_tag_allow_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_tag_block_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) search_profile_tag_prefer_error: Arc<Mutex<Option<SearchProfileServiceError>>>,
    pub(super) policy_set_create_calls: Arc<Mutex<Vec<PolicySetCreateCall>>>,
    pub(super) policy_set_update_calls: Arc<Mutex<Vec<PolicySetUpdateCall>>>,
    pub(super) policy_set_enable_calls: Arc<Mutex<Vec<PolicySetToggleCall>>>,
    pub(super) policy_set_disable_calls: Arc<Mutex<Vec<PolicySetToggleCall>>>,
    pub(super) policy_set_reorder_calls: Arc<Mutex<Vec<PolicySetReorderCall>>>,
    pub(super) policy_rule_create_calls: Arc<Mutex<Vec<PolicyRuleCreateParams>>>,
    pub(super) policy_rule_enable_calls: Arc<Mutex<Vec<PolicyRuleToggleCall>>>,
    pub(super) policy_rule_disable_calls: Arc<Mutex<Vec<PolicyRuleToggleCall>>>,
    pub(super) policy_rule_reorder_calls: Arc<Mutex<Vec<PolicyRuleReorderCall>>>,
    pub(super) policy_set_list_calls: Arc<Mutex<Vec<Uuid>>>,
    pub(super) policy_set_list_items: Arc<Mutex<Vec<PolicySetListItemResponse>>>,
    pub(super) policy_set_create_result: Arc<Mutex<Option<Result<Uuid, PolicyServiceError>>>>,
    pub(super) policy_set_update_result: Arc<Mutex<Option<Result<Uuid, PolicyServiceError>>>>,
    pub(super) policy_set_enable_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_set_disable_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_set_reorder_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_rule_create_result: Arc<Mutex<Option<Result<Uuid, PolicyServiceError>>>>,
    pub(super) policy_rule_enable_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_rule_disable_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_rule_reorder_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) policy_set_list_error: Arc<Mutex<Option<PolicyServiceError>>>,
    pub(super) search_page_list_response: Arc<Mutex<Option<SearchPageListResponse>>>,
    pub(super) search_page_fetch_response: Arc<Mutex<Option<SearchPageResponse>>>,
    pub(super) search_page_list_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) search_page_fetch_error: Arc<Mutex<Option<SearchRequestServiceError>>>,
    pub(super) torznab_auth_calls: Arc<Mutex<Vec<TorznabAuthCall>>>,
    pub(super) torznab_auth_result: Arc<Mutex<Option<TorznabAuthResult>>>,
    pub(super) torznab_download_prepare_calls: Arc<Mutex<Vec<TorznabDownloadPrepareCall>>>,
    pub(super) torznab_download_prepare_result: Arc<Mutex<Option<TorznabDownloadPrepareResult>>>,
    pub(super) torznab_category_list_calls: Arc<Mutex<usize>>,
    pub(super) torznab_category_list_result: Arc<Mutex<Option<TorznabCategoryListResult>>>,
    pub(super) torznab_feed_category_calls: Arc<Mutex<Vec<TorznabFeedCategoryIdsCall>>>,
    pub(super) torznab_feed_category_result: Arc<Mutex<Option<TorznabFeedCategoryResult>>>,
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
    pub(super) secret_metadata: Arc<Mutex<Vec<SecretMetadataResponse>>>,
    pub(super) secret_error: Arc<Mutex<Option<SecretServiceError>>>,
    pub(super) tag_calls: Arc<Mutex<Vec<(Uuid, String, String)>>>,
    pub(super) tag_list_items: Arc<Mutex<Vec<TagListItemResponse>>>,
    pub(super) tag_result: Arc<Mutex<Option<Result<Uuid, TagServiceError>>>>,
    pub(super) tag_error: Arc<Mutex<Option<TagServiceError>>>,
    pub(super) source_metadata_conflict_resolve_calls:
        Arc<Mutex<Vec<SourceMetadataConflictResolveCall>>>,
    pub(super) source_metadata_conflict_reopen_calls:
        Arc<Mutex<Vec<SourceMetadataConflictReopenCall>>>,
    pub(super) source_metadata_conflict_list_calls: Arc<Mutex<Vec<SourceMetadataConflictListCall>>>,
    pub(super) source_metadata_conflict_list_response:
        Arc<Mutex<Option<Vec<IndexerSourceMetadataConflictResponse>>>>,
    pub(super) source_metadata_conflict_error:
        Arc<Mutex<Option<SourceMetadataConflictServiceError>>>,
    pub(super) tracker_category_mapping_upsert_calls:
        Arc<Mutex<Vec<TrackerCategoryMappingUpsertCall>>>,
    pub(super) tracker_category_mapping_delete_calls:
        Arc<Mutex<Vec<TrackerCategoryMappingDeleteCall>>>,
    pub(super) media_domain_mapping_upsert_calls: Arc<Mutex<Vec<MediaDomainMappingUpsertCall>>>,
    pub(super) media_domain_mapping_delete_calls: Arc<Mutex<Vec<MediaDomainMappingDeleteCall>>>,
}

/// Snapshot of search request create inputs for assertions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchRequestCreateSnapshot {
    /// Optional actor identifier passed through the handler.
    pub(crate) actor_user_public_id: Option<Uuid>,
    /// Trimmed query text.
    pub(crate) query_text: String,
    /// Trimmed query type.
    pub(crate) query_type: String,
    /// Optional torznab mode.
    pub(crate) torznab_mode: Option<String>,
    /// Optional requested media domain key.
    pub(crate) requested_media_domain_key: Option<String>,
    /// Optional requested page size.
    pub(crate) page_size: Option<i32>,
    /// Optional season number.
    pub(crate) season_number: Option<i32>,
    /// Optional episode number.
    pub(crate) episode_number: Option<i32>,
    /// Optional normalized identifier types.
    pub(crate) identifier_types: Option<Vec<String>>,
    /// Optional normalized identifier values.
    pub(crate) identifier_values: Option<Vec<String>>,
    /// Optional Torznab category ids.
    pub(crate) torznab_cat_ids: Option<Vec<i32>>,
}

#[cfg(test)]
impl RecordingIndexers {
    pub(crate) fn set_torznab_auth_result(
        &self,
        result: Result<TorznabInstanceAuth, TorznabAccessError>,
    ) {
        *self.torznab_auth_result.lock().expect("lock") = Some(result);
    }

    pub(crate) fn set_torznab_category_list_result(
        &self,
        result: Result<Vec<TorznabCategory>, TorznabAccessError>,
    ) {
        *self.torznab_category_list_result.lock().expect("lock") = Some(result);
    }

    pub(crate) fn set_torznab_download_prepare_result(
        &self,
        result: Result<Option<String>, TorznabAccessError>,
    ) {
        *self.torznab_download_prepare_result.lock().expect("lock") = Some(result);
    }

    pub(crate) fn torznab_auth_calls(&self) -> Vec<(Uuid, String)> {
        self.torznab_auth_calls.lock().expect("lock").clone()
    }

    pub(crate) fn torznab_download_prepare_calls(&self) -> Vec<(Uuid, Uuid)> {
        self.torznab_download_prepare_calls
            .lock()
            .expect("lock")
            .clone()
    }

    pub(crate) fn set_torznab_feed_category_result(
        &self,
        result: Result<Vec<i32>, TorznabAccessError>,
    ) {
        *self.torznab_feed_category_result.lock().expect("lock") = Some(result);
    }

    pub(crate) fn set_source_metadata_conflict_list_response(
        &self,
        conflicts: Vec<IndexerSourceMetadataConflictResponse>,
    ) {
        *self
            .source_metadata_conflict_list_response
            .lock()
            .expect("lock") = Some(conflicts);
    }

    pub(crate) fn set_source_metadata_conflict_error(
        &self,
        error: SourceMetadataConflictServiceError,
    ) {
        *self.source_metadata_conflict_error.lock().expect("lock") = Some(error);
    }

    pub(crate) fn set_search_page_list_response(&self, response: SearchPageListResponse) {
        *self.search_page_list_response.lock().expect("lock") = Some(response);
    }

    pub(crate) fn set_search_page_fetch_response(&self, response: SearchPageResponse) {
        *self.search_page_fetch_response.lock().expect("lock") = Some(response);
    }

    pub(crate) fn set_search_page_fetch_error(&self, error: SearchRequestServiceError) {
        *self.search_page_fetch_error.lock().expect("lock") = Some(error);
    }

    pub(crate) fn search_request_snapshots(&self) -> Vec<SearchRequestCreateSnapshot> {
        self.search_request_calls.lock().expect("lock").clone()
    }
}

#[cfg(test)]
#[async_trait]
impl IndexerFacade for RecordingIndexers {
    async fn indexer_definition_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
        self.indexer_definition_list_calls
            .lock()
            .expect("lock")
            .push(actor_user_public_id);

        if let Some(result) = take_locked(&self.indexer_definition_list_result) {
            return result;
        }

        Ok(Vec::new())
    }

    async fn indexer_definition_import_cardigann(
        &self,
        actor_user_public_id: Uuid,
        yaml_payload: &str,
        is_deprecated: Option<bool>,
    ) -> Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError> {
        self.indexer_definition_import_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                yaml_payload.to_string(),
                is_deprecated,
            ));

        if let Some(result) = take_locked(&self.indexer_definition_import_result) {
            return result;
        }

        Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Storage,
        ))
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
        actor_user_public_id: Uuid,
        display_name: &str,
        is_default: Option<bool>,
        page_size: Option<i32>,
        default_media_domain_key: Option<&str>,
        user_public_id: Option<Uuid>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        self.search_profile_create_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                display_name.to_string(),
                is_default,
                page_size,
                default_media_domain_key.map(str::to_string),
                user_public_id,
            ));

        if let Some(result) = take_locked(&self.search_profile_create_result) {
            return result;
        }

        Ok(DEFAULT_SEARCH_PROFILE_PUBLIC_ID)
    }

    async fn search_profile_update(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: Option<&str>,
        page_size: Option<i32>,
    ) -> Result<Uuid, SearchProfileServiceError> {
        self.search_profile_update_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                display_name.map(str::to_string),
                page_size,
            ));

        let error = self
            .search_profile_update_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(search_profile_public_id)
    }

    async fn search_profile_set_default(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        page_size: Option<i32>,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_default_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, search_profile_public_id, page_size));

        let error = self
            .search_profile_default_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_set_default_domain(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        default_media_domain_key: Option<&str>,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_default_domain_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                default_media_domain_key.map(str::to_string),
            ));

        let error = self
            .search_profile_default_domain_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_set_domain_allowlist(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        media_domain_keys: &[String],
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_domain_allowlist_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                media_domain_keys.to_vec(),
            ));

        let error = self
            .search_profile_domain_allowlist_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_add_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_add_policy_set_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                policy_set_public_id,
            ));

        let error = self
            .search_profile_add_policy_set_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_remove_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_remove_policy_set_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                policy_set_public_id,
            ));

        let error = self
            .search_profile_remove_policy_set_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_indexer_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_indexer_allow_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                indexer_instance_public_ids.to_vec(),
            ));

        let error = self
            .search_profile_indexer_allow_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_indexer_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_indexer_block_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                indexer_instance_public_ids.to_vec(),
            ));

        let error = self
            .search_profile_indexer_block_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_tag_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_tag_allow_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids.map(ToOwned::to_owned),
                tag_keys.map(ToOwned::to_owned),
            ));

        let error = self
            .search_profile_tag_allow_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_tag_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_tag_block_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids.map(ToOwned::to_owned),
                tag_keys.map(ToOwned::to_owned),
            ));

        let error = self
            .search_profile_tag_block_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_tag_prefer(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError> {
        self.search_profile_tag_prefer_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                tag_public_ids.map(ToOwned::to_owned),
                tag_keys.map(ToOwned::to_owned),
            ));

        let error = self
            .search_profile_tag_prefer_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn search_profile_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<SearchProfileListItemResponse>, SearchProfileServiceError> {
        self.search_profile_list_calls
            .lock()
            .expect("lock")
            .push(actor_user_public_id);

        let error = self.search_profile_list_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(self.search_profile_list_items.lock().expect("lock").clone())
    }

    async fn policy_set_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        scope: &str,
        enabled: Option<bool>,
    ) -> Result<Uuid, PolicyServiceError> {
        self.policy_set_create_calls.lock().expect("lock").push((
            actor_user_public_id,
            display_name.to_string(),
            scope.to_string(),
            enabled,
        ));

        if let Some(result) = take_locked(&self.policy_set_create_result) {
            return result;
        }

        Ok(Uuid::new_v4())
    }

    async fn policy_set_update(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Uuid, PolicyServiceError> {
        self.policy_set_update_calls.lock().expect("lock").push((
            actor_user_public_id,
            policy_set_public_id,
            display_name.map(str::to_string),
        ));

        if let Some(result) = take_locked(&self.policy_set_update_result) {
            return result;
        }

        Ok(policy_set_public_id)
    }

    async fn policy_set_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        self.policy_set_enable_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, policy_set_public_id));

        if let Some(error) = take_locked(&self.policy_set_enable_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_set_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        self.policy_set_disable_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, policy_set_public_id));

        if let Some(error) = take_locked(&self.policy_set_disable_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_set_reorder(
        &self,
        actor_user_public_id: Uuid,
        ordered_policy_set_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        self.policy_set_reorder_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, ordered_policy_set_public_ids.to_vec()));

        if let Some(error) = take_locked(&self.policy_set_reorder_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_rule_create(
        &self,
        params: PolicyRuleCreateParams,
    ) -> Result<Uuid, PolicyServiceError> {
        self.policy_rule_create_calls
            .lock()
            .expect("lock")
            .push(params);

        if let Some(result) = take_locked(&self.policy_rule_create_result) {
            return result;
        }

        Ok(Uuid::new_v4())
    }

    async fn policy_rule_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        self.policy_rule_enable_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, policy_rule_public_id));

        if let Some(error) = take_locked(&self.policy_rule_enable_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_rule_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        self.policy_rule_disable_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, policy_rule_public_id));

        if let Some(error) = take_locked(&self.policy_rule_disable_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_rule_reorder(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        ordered_policy_rule_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        self.policy_rule_reorder_calls.lock().expect("lock").push((
            actor_user_public_id,
            policy_set_public_id,
            ordered_policy_rule_public_ids.to_vec(),
        ));

        if let Some(error) = take_locked(&self.policy_rule_reorder_error) {
            return Err(error);
        }

        Ok(())
    }

    async fn policy_set_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<PolicySetListItemResponse>, PolicyServiceError> {
        self.policy_set_list_calls
            .lock()
            .expect("lock")
            .push(actor_user_public_id);

        if let Some(error) = take_locked(&self.policy_set_list_error) {
            return Err(error);
        }

        Ok(self.policy_set_list_items.lock().expect("lock").clone())
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
            page_size: params.page_size,
            season_number: params.season_number,
            episode_number: params.episode_number,
            identifier_types: params.identifier_types.map(ToOwned::to_owned),
            identifier_values: params.identifier_values.map(ToOwned::to_owned),
            torznab_cat_ids: params.torznab_cat_ids.map(ToOwned::to_owned),
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

    async fn import_job_create(
        &self,
        actor_user_public_id: Uuid,
        source: &str,
        is_dry_run: Option<bool>,
        target_search_profile_public_id: Option<Uuid>,
        target_torznab_instance_public_id: Option<Uuid>,
    ) -> Result<Uuid, ImportJobServiceError> {
        let error = self.import_job_create_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        self.import_job_create_calls.lock().expect("lock").push((
            actor_user_public_id,
            source.to_string(),
            is_dry_run,
            target_search_profile_public_id,
            target_torznab_instance_public_id,
        ));
        Ok(DEFAULT_IMPORT_JOB_PUBLIC_ID)
    }

    async fn import_job_run_prowlarr_api(
        &self,
        import_job_public_id: Uuid,
        prowlarr_url: &str,
        prowlarr_api_key_secret_public_id: Uuid,
    ) -> Result<(), ImportJobServiceError> {
        let error = self
            .import_job_run_prowlarr_api_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        self.import_job_run_prowlarr_api_calls
            .lock()
            .expect("lock")
            .push((
                import_job_public_id,
                prowlarr_url.to_string(),
                prowlarr_api_key_secret_public_id,
            ));
        Ok(())
    }

    async fn import_job_run_prowlarr_backup(
        &self,
        import_job_public_id: Uuid,
        backup_blob_ref: &str,
    ) -> Result<(), ImportJobServiceError> {
        let error = self
            .import_job_run_prowlarr_backup_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        self.import_job_run_prowlarr_backup_calls
            .lock()
            .expect("lock")
            .push((import_job_public_id, backup_blob_ref.to_string()));
        Ok(())
    }

    async fn import_job_get_status(
        &self,
        _import_job_public_id: Uuid,
    ) -> Result<ImportJobStatusResponse, ImportJobServiceError> {
        let error = self.import_job_status_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self.import_job_status_response.lock().expect("lock").take();
        Ok(response.unwrap_or_else(|| ImportJobStatusResponse {
            status: "pending".to_string(),
            result_total: 0,
            result_imported_ready: 0,
            result_imported_needs_secret: 0,
            result_imported_test_failed: 0,
            result_unmapped_definition: 0,
            result_skipped_duplicate: 0,
        }))
    }

    async fn import_job_list_results(
        &self,
        _import_job_public_id: Uuid,
    ) -> Result<Vec<ImportJobResultResponse>, ImportJobServiceError> {
        let error = self.import_job_results_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        let response = self
            .import_job_results_response
            .lock()
            .expect("lock")
            .take();
        Ok(response.unwrap_or_default())
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
        Ok(response.unwrap_or_else(|| SearchPageListResponse {
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

    async fn tag_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<TagListItemResponse>, TagServiceError> {
        let tag_error = self.tag_error.lock().expect("lock poisoned").take();
        if let Some(error) = tag_error {
            return Err(error);
        }
        Ok(self.tag_list_items.lock().expect("lock poisoned").clone())
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

    async fn source_metadata_conflict_list(
        &self,
        actor_user_public_id: Uuid,
        include_resolved: Option<bool>,
        limit: Option<i32>,
    ) -> Result<
        Vec<crate::models::IndexerSourceMetadataConflictResponse>,
        SourceMetadataConflictServiceError,
    > {
        self.source_metadata_conflict_list_calls
            .lock()
            .expect("lock poisoned")
            .push((actor_user_public_id, include_resolved, limit));
        let error = self
            .source_metadata_conflict_error
            .lock()
            .expect("lock poisoned")
            .take();
        if let Some(error) = error {
            return Err(error);
        }
        Ok(self
            .source_metadata_conflict_list_response
            .lock()
            .expect("lock poisoned")
            .clone()
            .unwrap_or_default())
    }

    async fn source_metadata_conflict_resolve(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution: &str,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        self.source_metadata_conflict_resolve_calls
            .lock()
            .expect("lock poisoned")
            .push((
                actor_user_public_id,
                conflict_id,
                resolution.to_string(),
                resolution_note.map(str::to_string),
            ));
        let error = self
            .source_metadata_conflict_error
            .lock()
            .expect("lock poisoned")
            .take();
        if let Some(error) = error {
            return Err(error);
        }
        Ok(())
    }

    async fn source_metadata_conflict_reopen(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        self.source_metadata_conflict_reopen_calls
            .lock()
            .expect("lock poisoned")
            .push((
                actor_user_public_id,
                conflict_id,
                resolution_note.map(str::to_string),
            ));
        let error = self
            .source_metadata_conflict_error
            .lock()
            .expect("lock poisoned")
            .take();
        if let Some(error) = error {
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
        actor_user_public_id: Uuid,
        display_name: &str,
        mode: &str,
    ) -> Result<Uuid, RoutingPolicyServiceError> {
        self.routing_policy_create_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                display_name.to_string(),
                mode.to_string(),
            ));

        if let Some(result) = take_locked(&self.routing_policy_create_result) {
            return result;
        }

        Ok(DEFAULT_ROUTING_POLICY_PUBLIC_ID)
    }

    async fn routing_policy_set_param(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        value_plain: Option<&str>,
        value_int: Option<i32>,
        value_bool: Option<bool>,
    ) -> Result<(), RoutingPolicyServiceError> {
        self.routing_policy_set_param_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                routing_policy_public_id,
                param_key.to_string(),
                value_plain.map(str::to_string),
                value_int,
                value_bool,
            ));

        let error = self
            .routing_policy_set_param_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn routing_policy_bind_secret(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        secret_public_id: Uuid,
    ) -> Result<(), RoutingPolicyServiceError> {
        self.routing_policy_bind_secret_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                routing_policy_public_id,
                param_key.to_string(),
                secret_public_id,
            ));

        let error = self
            .routing_policy_bind_secret_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn routing_policy_get(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
    ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
        self.routing_policy_get_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, routing_policy_public_id));

        if let Some(result) = take_locked(&self.routing_policy_get_result) {
            return result;
        }

        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }

    async fn routing_policy_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<RoutingPolicyListItemResponse>, RoutingPolicyServiceError> {
        if let Some(result) = take_locked(&self.routing_policy_list_result) {
            return result;
        }

        Ok(Vec::new())
    }

    async fn rate_limit_policy_create(
        &self,
        _actor_user_public_id: Uuid,
        display_name: &str,
        rpm: i32,
        burst: i32,
        concurrent: i32,
    ) -> Result<Uuid, RateLimitPolicyServiceError> {
        let error = self.rate_limit_create_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        self.rate_limit_create_calls.lock().expect("lock").push((
            display_name.to_string(),
            rpm,
            burst,
            concurrent,
        ));
        Ok(DEFAULT_RATE_LIMIT_POLICY_PUBLIC_ID)
    }

    async fn rate_limit_policy_update(
        &self,
        _actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
        display_name: Option<&str>,
        rpm: Option<i32>,
        burst: Option<i32>,
        concurrent: Option<i32>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let error = self.rate_limit_update_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        self.rate_limit_update_calls.lock().expect("lock").push((
            rate_limit_policy_public_id,
            display_name.map(str::to_string),
            rpm,
            burst,
            concurrent,
        ));
        Ok(())
    }

    async fn rate_limit_policy_soft_delete(
        &self,
        _actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let error = self.rate_limit_delete_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        self.rate_limit_deleted_calls
            .lock()
            .expect("lock")
            .push(rate_limit_policy_public_id);
        Ok(())
    }

    async fn indexer_instance_set_rate_limit_policy(
        &self,
        _actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let error = self
            .indexer_rate_limit_assignment_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        self.indexer_rate_limit_assignment_calls
            .lock()
            .expect("lock")
            .push((indexer_instance_public_id, rate_limit_policy_public_id));
        Ok(())
    }

    async fn routing_policy_set_rate_limit_policy(
        &self,
        _actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError> {
        let error = self
            .routing_rate_limit_assignment_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        self.routing_rate_limit_assignment_calls
            .lock()
            .expect("lock")
            .push((routing_policy_public_id, rate_limit_policy_public_id));
        Ok(())
    }

    async fn rate_limit_policy_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<RateLimitPolicyListItemResponse>, RateLimitPolicyServiceError> {
        let error = self.rate_limit_list_error.lock().expect("lock").take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(self.rate_limit_list_items.lock().expect("lock").clone())
    }

    async fn tracker_category_mapping_upsert(
        &self,
        params: crate::app::indexers::TrackerCategoryMappingUpsertParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        self.tracker_category_mapping_upsert_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                params.torznab_instance_public_id,
                params.indexer_definition_upstream_slug.map(str::to_string),
                params.indexer_instance_public_id,
                params.tracker_category,
                params.tracker_subcategory,
                params.torznab_cat_id,
                params.media_domain_key.map(str::to_string),
            ));
        Ok(())
    }

    async fn tracker_category_mapping_delete(
        &self,
        params: crate::app::indexers::TrackerCategoryMappingDeleteParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        self.tracker_category_mapping_delete_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                params.torznab_instance_public_id,
                params.indexer_definition_upstream_slug.map(str::to_string),
                params.indexer_instance_public_id,
                params.tracker_category,
                params.tracker_subcategory,
            ));
        Ok(())
    }

    async fn media_domain_mapping_upsert(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
        is_primary: Option<bool>,
    ) -> Result<(), CategoryMappingServiceError> {
        self.media_domain_mapping_upsert_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                actor_user_public_id,
                media_domain_key.to_string(),
                torznab_cat_id,
                is_primary,
            ));
        Ok(())
    }

    async fn media_domain_mapping_delete(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
    ) -> Result<(), CategoryMappingServiceError> {
        self.media_domain_mapping_delete_calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                actor_user_public_id,
                media_domain_key.to_string(),
                torznab_cat_id,
            ));
        Ok(())
    }

    async fn torznab_instance_create(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: &str,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        self.torznab_instance_create_calls
            .lock()
            .expect("lock")
            .push((
                actor_user_public_id,
                search_profile_public_id,
                display_name.to_string(),
            ));

        if let Some(result) = take_locked(&self.torznab_instance_create_result) {
            return result;
        }

        Ok(TorznabInstanceCredentials {
            torznab_instance_public_id: DEFAULT_TORZNAB_INSTANCE_PUBLIC_ID,
            api_key_plaintext: "torznab-api-key".to_string(),
        })
    }

    async fn torznab_instance_rotate_key(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError> {
        self.torznab_instance_rotate_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, torznab_instance_public_id));

        if let Some(result) = take_locked(&self.torznab_instance_rotate_result) {
            return result;
        }

        Ok(TorznabInstanceCredentials {
            torznab_instance_public_id,
            api_key_plaintext: "rotated-torznab-api-key".to_string(),
        })
    }

    async fn torznab_instance_enable_disable(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
        is_enabled: bool,
    ) -> Result<(), TorznabInstanceServiceError> {
        self.torznab_instance_state_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, torznab_instance_public_id, is_enabled));

        let error = self
            .torznab_instance_state_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
    }

    async fn torznab_instance_soft_delete(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<(), TorznabInstanceServiceError> {
        self.torznab_instance_delete_calls
            .lock()
            .expect("lock")
            .push((actor_user_public_id, torznab_instance_public_id));

        let error = self
            .torznab_instance_delete_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(())
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

    async fn secret_metadata_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<SecretMetadataResponse>, SecretServiceError> {
        let secret_error = self.secret_error.lock().expect("lock poisoned").take();
        if let Some(error) = secret_error {
            return Err(error);
        }
        Ok(self.secret_metadata.lock().expect("lock poisoned").clone())
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

    async fn torznab_instance_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<TorznabInstanceListItemResponse>, TorznabInstanceServiceError> {
        let error = self
            .torznab_instance_list_error
            .lock()
            .expect("lock")
            .take();
        if let Some(error) = error {
            return Err(error);
        }

        Ok(self
            .torznab_instance_list_items
            .lock()
            .expect("lock")
            .clone())
    }

    async fn torznab_instance_authenticate(
        &self,
        torznab_instance_public_id: Uuid,
        api_key_plaintext: &str,
    ) -> Result<TorznabInstanceAuth, TorznabAccessError> {
        self.torznab_auth_calls
            .lock()
            .expect("lock")
            .push((torznab_instance_public_id, api_key_plaintext.to_string()));

        if let Some(result) = take_locked(&self.torznab_auth_result) {
            return result;
        }

        Err(TorznabAccessError::new(
            TorznabAccessErrorKind::Unauthorized,
        ))
    }

    async fn torznab_download_prepare(
        &self,
        torznab_instance_public_id: Uuid,
        canonical_torrent_source_public_id: Uuid,
    ) -> Result<Option<String>, TorznabAccessError> {
        self.torznab_download_prepare_calls
            .lock()
            .expect("lock")
            .push((
                torznab_instance_public_id,
                canonical_torrent_source_public_id,
            ));

        if let Some(result) = take_locked(&self.torznab_download_prepare_result) {
            return result;
        }

        Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage))
    }

    async fn torznab_category_list(&self) -> Result<Vec<TorznabCategory>, TorznabAccessError> {
        *self.torznab_category_list_calls.lock().expect("lock") += 1;

        if let Some(result) = take_locked(&self.torznab_category_list_result) {
            return result;
        }

        Ok(Vec::new())
    }

    async fn torznab_feed_category_ids(
        &self,
        torznab_instance_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        tracker_category: Option<i32>,
        tracker_subcategory: Option<i32>,
    ) -> Result<Vec<i32>, TorznabAccessError> {
        self.torznab_feed_category_calls
            .lock()
            .expect("lock")
            .push((
                torznab_instance_public_id,
                indexer_instance_public_id,
                tracker_category,
                tracker_subcategory,
            ));

        if let Some(result) = take_locked(&self.torznab_feed_category_result) {
            return result;
        }

        Ok(Vec::new())
    }
}

pub(crate) fn indexer_test_state(
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

pub(crate) async fn parse_problem(response: Response) -> ProblemDetails {
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
