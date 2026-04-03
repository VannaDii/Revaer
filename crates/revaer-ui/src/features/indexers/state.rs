//! Indexer admin UI state.
//!
//! # Design
//! - Keep all mutable form values local to the feature so admin actions remain isolated.
//! - Represent optional identifiers and numbers as strings until submission time.
//! - Capture operation output as append-only records for quick operator feedback.

use crate::models::{
    CardigannDefinitionImportResponse, ImportJobResultResponse, ImportJobStatusResponse,
    IndexerBackupSnapshot, IndexerBackupUnresolvedSecretBinding,
    IndexerConnectivityProfileResponse, IndexerDefinitionResponse, IndexerHealthEventResponse,
    IndexerHealthNotificationHookResponse, IndexerInstanceListItemResponse,
    IndexerSourceMetadataConflictResponse, IndexerSourceReputationResponse,
    PolicySetListItemResponse, RateLimitPolicyListItemResponse, RoutingPolicyDetailResponse,
    RoutingPolicyListItemResponse, SearchProfileListItemResponse, SecretMetadataResponse,
    TagListItemResponse, TorznabInstanceListItemResponse,
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OperationRecord {
    pub title: String,
    pub body: String,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct IndexersDraft {
    pub definitions_filter: String,
    pub tag_key: String,
    pub tag_display_name: String,
    pub tag_public_id: String,
    pub health_notification_hook_public_id: String,
    pub health_notification_channel: String,
    pub health_notification_display_name: String,
    pub health_notification_status_threshold: String,
    pub health_notification_webhook_url: String,
    pub health_notification_email: String,
    pub health_notification_is_enabled: bool,
    pub secret_public_id: String,
    pub secret_type: String,
    pub secret_display_name: String,
    pub secret_value: String,
    pub secret_new_value: String,
    pub routing_display_name: String,
    pub routing_mode: String,
    pub routing_policy_public_id: String,
    pub routing_param_key: String,
    pub routing_param_plain: String,
    pub routing_param_int: String,
    pub routing_param_bool: String,
    pub routing_secret_public_id: String,
    pub rate_limit_public_id: String,
    pub rate_limit_display_name: String,
    pub rate_limit_rpm: String,
    pub rate_limit_burst: String,
    pub rate_limit_concurrent: String,
    pub rate_limit_indexer_public_id: String,
    pub rate_limit_routing_public_id: String,
    pub category_mapping_definition_upstream_slug: String,
    pub category_mapping_torznab_instance_public_id: String,
    pub category_mapping_indexer_instance_public_id: String,
    pub category_mapping_tracker_category: String,
    pub category_mapping_tracker_subcategory: String,
    pub category_mapping_torznab_cat_id: String,
    pub category_mapping_media_domain_key: String,
    pub indexer_definition_upstream_slug: String,
    pub indexer_display_name: String,
    pub indexer_priority: String,
    pub indexer_trust_tier_key: String,
    pub indexer_routing_policy_public_id: String,
    pub indexer_instance_public_id: String,
    pub indexer_is_enabled: bool,
    pub indexer_enable_rss: bool,
    pub indexer_enable_automatic_search: bool,
    pub indexer_enable_interactive_search: bool,
    pub indexer_rss_interval_seconds: String,
    pub indexer_rss_recent_limit: String,
    pub indexer_reputation_window: String,
    pub indexer_reputation_limit: String,
    pub indexer_health_event_limit: String,
    pub indexer_rss_item_guid: String,
    pub indexer_rss_infohash_v1: String,
    pub indexer_rss_infohash_v2: String,
    pub indexer_rss_magnet_hash: String,
    pub indexer_media_domain_keys: String,
    pub indexer_tag_keys: String,
    pub indexer_field_name: String,
    pub indexer_field_plain: String,
    pub indexer_field_int: String,
    pub indexer_field_decimal: String,
    pub indexer_field_bool: String,
    pub indexer_field_secret_public_id: String,
    pub cf_reset_reason: String,
    pub test_ok: bool,
    pub test_error_class: String,
    pub test_error_code: String,
    pub test_detail: String,
    pub test_result_count: String,
    pub search_profile_display_name: String,
    pub search_profile_public_id: String,
    pub search_profile_is_default: bool,
    pub search_profile_page_size: String,
    pub search_profile_default_media_domain_key: String,
    pub search_profile_media_domain_keys: String,
    pub search_profile_policy_set_public_id: String,
    pub search_profile_indexer_public_ids: String,
    pub search_profile_tag_keys_allow: String,
    pub search_profile_tag_keys_block: String,
    pub search_profile_tag_keys_prefer: String,
    pub policy_set_display_name: String,
    pub policy_set_scope: String,
    pub policy_set_public_id: String,
    pub policy_set_user_public_id: String,
    pub policy_rule_type: String,
    pub policy_match_field: String,
    pub policy_match_operator: String,
    pub policy_sort_order: String,
    pub policy_match_value_text: String,
    pub policy_match_value_int: String,
    pub policy_match_value_uuid: String,
    pub policy_action: String,
    pub policy_severity: String,
    pub policy_is_case_insensitive: bool,
    pub policy_rationale: String,
    pub policy_value_set_text: String,
    pub import_job_source: String,
    pub import_job_payload_format: String,
    pub cardigann_yaml_payload: String,
    pub cardigann_is_deprecated: bool,
    pub import_job_public_id: String,
    pub source_conflict_id: String,
    pub source_conflict_limit: String,
    pub source_conflict_resolution: String,
    pub source_conflict_resolution_note: String,
    pub source_conflict_include_resolved: bool,
    pub prowlarr_base_url: String,
    pub prowlarr_api_key: String,
    pub import_dry_run: bool,
    pub prowlarr_backup_payload: String,
    pub backup_snapshot_payload: String,
    pub torznab_search_profile_public_id: String,
    pub torznab_display_name: String,
    pub torznab_instance_public_id: String,
    pub torznab_is_enabled: bool,
}

impl Default for IndexersDraft {
    fn default() -> Self {
        Self {
            definitions_filter: String::new(),
            tag_key: String::new(),
            tag_display_name: String::new(),
            tag_public_id: String::new(),
            health_notification_hook_public_id: String::new(),
            health_notification_channel: "webhook".to_string(),
            health_notification_display_name: String::new(),
            health_notification_status_threshold: "failing".to_string(),
            health_notification_webhook_url: String::new(),
            health_notification_email: String::new(),
            health_notification_is_enabled: true,
            secret_public_id: String::new(),
            secret_type: "api_key".to_string(),
            secret_display_name: String::new(),
            secret_value: String::new(),
            secret_new_value: String::new(),
            routing_display_name: String::new(),
            routing_mode: "direct".to_string(),
            routing_policy_public_id: String::new(),
            routing_param_key: String::new(),
            routing_param_plain: String::new(),
            routing_param_int: String::new(),
            routing_param_bool: String::new(),
            routing_secret_public_id: String::new(),
            rate_limit_public_id: String::new(),
            rate_limit_display_name: String::new(),
            rate_limit_rpm: "60".to_string(),
            rate_limit_burst: "10".to_string(),
            rate_limit_concurrent: "4".to_string(),
            rate_limit_indexer_public_id: String::new(),
            rate_limit_routing_public_id: String::new(),
            category_mapping_definition_upstream_slug: String::new(),
            category_mapping_torznab_instance_public_id: String::new(),
            category_mapping_indexer_instance_public_id: String::new(),
            category_mapping_tracker_category: String::new(),
            category_mapping_tracker_subcategory: String::new(),
            category_mapping_torznab_cat_id: String::new(),
            category_mapping_media_domain_key: String::new(),
            indexer_definition_upstream_slug: String::new(),
            indexer_display_name: String::new(),
            indexer_priority: "50".to_string(),
            indexer_trust_tier_key: "public".to_string(),
            indexer_routing_policy_public_id: String::new(),
            indexer_instance_public_id: String::new(),
            indexer_is_enabled: true,
            indexer_enable_rss: true,
            indexer_enable_automatic_search: true,
            indexer_enable_interactive_search: true,
            indexer_rss_interval_seconds: "900".to_string(),
            indexer_rss_recent_limit: "25".to_string(),
            indexer_reputation_window: "1h".to_string(),
            indexer_reputation_limit: "10".to_string(),
            indexer_health_event_limit: "20".to_string(),
            indexer_rss_item_guid: String::new(),
            indexer_rss_infohash_v1: String::new(),
            indexer_rss_infohash_v2: String::new(),
            indexer_rss_magnet_hash: String::new(),
            indexer_media_domain_keys: String::new(),
            indexer_tag_keys: String::new(),
            indexer_field_name: String::new(),
            indexer_field_plain: String::new(),
            indexer_field_int: String::new(),
            indexer_field_decimal: String::new(),
            indexer_field_bool: String::new(),
            indexer_field_secret_public_id: String::new(),
            cf_reset_reason: "operator_reset".to_string(),
            test_ok: true,
            test_error_class: String::new(),
            test_error_code: String::new(),
            test_detail: String::new(),
            test_result_count: String::new(),
            search_profile_display_name: String::new(),
            search_profile_public_id: String::new(),
            search_profile_is_default: false,
            search_profile_page_size: "50".to_string(),
            search_profile_default_media_domain_key: String::new(),
            search_profile_media_domain_keys: String::new(),
            search_profile_policy_set_public_id: String::new(),
            search_profile_indexer_public_ids: String::new(),
            search_profile_tag_keys_allow: String::new(),
            search_profile_tag_keys_block: String::new(),
            search_profile_tag_keys_prefer: String::new(),
            policy_set_display_name: String::new(),
            policy_set_scope: "global".to_string(),
            policy_set_public_id: String::new(),
            policy_set_user_public_id: String::new(),
            policy_rule_type: "allow_title_regex".to_string(),
            policy_match_field: "title".to_string(),
            policy_match_operator: "regex".to_string(),
            policy_sort_order: "10".to_string(),
            policy_match_value_text: String::new(),
            policy_match_value_int: String::new(),
            policy_match_value_uuid: String::new(),
            policy_action: "prefer".to_string(),
            policy_severity: "soft".to_string(),
            policy_is_case_insensitive: false,
            policy_rationale: String::new(),
            policy_value_set_text: String::new(),
            import_job_source: "prowlarr".to_string(),
            import_job_payload_format: "prowlarr_indexer_json_v1".to_string(),
            cardigann_yaml_payload: String::new(),
            cardigann_is_deprecated: false,
            import_job_public_id: String::new(),
            source_conflict_id: String::new(),
            source_conflict_limit: "20".to_string(),
            source_conflict_resolution: "kept_existing".to_string(),
            source_conflict_resolution_note: String::new(),
            source_conflict_include_resolved: false,
            prowlarr_base_url: String::new(),
            prowlarr_api_key: String::new(),
            import_dry_run: true,
            prowlarr_backup_payload: String::new(),
            backup_snapshot_payload: String::new(),
            torznab_search_profile_public_id: String::new(),
            torznab_display_name: String::new(),
            torznab_instance_public_id: String::new(),
            torznab_is_enabled: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct DefinitionsState {
    pub definitions: Vec<IndexerDefinitionResponse>,
    pub loaded: bool,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct RoutingPolicyState {
    pub detail: Option<RoutingPolicyDetailResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct RoutingPolicyInventoryState {
    pub items: Vec<RoutingPolicyListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct RateLimitInventoryState {
    pub items: Vec<RateLimitPolicyListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct IndexerInstanceInventoryState {
    pub items: Vec<IndexerInstanceListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct SearchProfileInventoryState {
    pub items: Vec<SearchProfileListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct PolicySetInventoryState {
    pub items: Vec<PolicySetListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct TorznabInstanceInventoryState {
    pub items: Vec<TorznabInstanceListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct HealthNotificationHooksState {
    pub hooks: Vec<IndexerHealthNotificationHookResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct TagInventoryState {
    pub items: Vec<TagListItemResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct SecretInventoryState {
    pub items: Vec<SecretMetadataResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct ImportJobState {
    pub status: Option<ImportJobStatusResponse>,
    pub results: Vec<ImportJobResultResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct CardigannImportState {
    pub summary: Option<CardigannDefinitionImportResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct SourceMetadataConflictsState {
    pub items: Vec<IndexerSourceMetadataConflictResponse>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AppSyncProvisionSummary {
    pub search_profile_public_id: Uuid,
    pub created_search_profile: bool,
    pub torznab_instance_public_id: Uuid,
    pub torznab_api_key_plaintext: String,
    pub default_media_domain_key: Option<String>,
    pub media_domain_keys: Vec<String>,
    pub allowed_indexer_public_ids: Vec<Uuid>,
    pub allowed_tag_keys: Vec<String>,
    pub blocked_tag_keys: Vec<String>,
    pub preferred_tag_keys: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct AppSyncState {
    pub summary: Option<AppSyncProvisionSummary>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct HealthEventsState {
    pub items: Vec<IndexerHealthEventResponse>,
}

#[derive(Clone, PartialEq, Default)]
pub(crate) struct ConnectivityInsightsState {
    pub profile: Option<IndexerConnectivityProfileResponse>,
    pub reputation_items: Vec<IndexerSourceReputationResponse>,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct BackupState {
    pub snapshot: Option<IndexerBackupSnapshot>,
    pub unresolved_secret_bindings: Vec<IndexerBackupUnresolvedSecretBinding>,
}
