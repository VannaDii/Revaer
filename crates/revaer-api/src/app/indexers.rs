//! Indexer application facade and error types.
//!
//! # Design
//! - Expose a narrow async trait for indexer operations used by HTTP handlers.
//! - Keep error messages constant; attach error codes and context as fields.
//! - Ensure callers provide the actor identity for audit and authorization checks.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{
    CardigannDefinitionImportResponse, ImportJobResultResponse, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerBackupRestoreResponse, IndexerCfStateResponse,
    IndexerConnectivityProfileResponse, IndexerDefinitionResponse, IndexerHealthEventResponse,
    IndexerHealthNotificationHookResponse, IndexerInstanceListItemResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemResponse, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerSourceMetadataConflictResponse, IndexerSourceReputationResponse,
    RateLimitPolicyListItemResponse, RoutingPolicyDetailResponse, RoutingPolicyListItemResponse,
    SearchPageListResponse, SearchPageResponse, SearchRequestCreateResponse,
    SecretMetadataResponse, TagListItemResponse,
};

/// Parameters for updating an indexer instance.
#[derive(Debug, Clone)]
pub struct IndexerInstanceUpdateParams<'a> {
    /// Actor performing the update.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Optional display name.
    pub display_name: Option<&'a str>,
    /// Optional priority override.
    pub priority: Option<i32>,
    /// Optional trust tier key.
    pub trust_tier_key: Option<&'a str>,
    /// Optional routing policy binding.
    pub routing_policy_public_id: Option<Uuid>,
    /// Enable or disable the instance.
    pub is_enabled: Option<bool>,
    /// Enable or disable RSS.
    pub enable_rss: Option<bool>,
    /// Enable or disable automatic search.
    pub enable_automatic_search: Option<bool>,
    /// Enable or disable interactive search.
    pub enable_interactive_search: Option<bool>,
}

/// Parameters for setting a field value on an indexer instance.
#[derive(Debug, Clone)]
pub struct IndexerInstanceFieldValueParams<'a> {
    /// Actor performing the update.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Field name to update.
    pub field_name: &'a str,
    /// Optional plain value.
    pub value_plain: Option<&'a str>,
    /// Optional integer value.
    pub value_int: Option<i32>,
    /// Optional decimal value as text.
    pub value_decimal: Option<&'a str>,
    /// Optional boolean value.
    pub value_bool: Option<bool>,
}

/// Parameters for resetting Cloudflare mitigation state.
#[derive(Debug, Clone)]
pub struct IndexerCfStateResetParams<'a> {
    /// Actor performing the reset.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Human-readable reason (constant strings only; no user PII).
    pub reason: &'a str,
}

/// Parameters for finalizing an indexer instance test.
#[derive(Debug, Clone)]
pub struct IndexerInstanceTestFinalizeParams<'a> {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Whether the test succeeded.
    pub ok: bool,
    /// Optional error class label.
    pub error_class: Option<&'a str>,
    /// Optional error code string.
    pub error_code: Option<&'a str>,
    /// Optional detail string.
    pub detail: Option<&'a str>,
    /// Optional result count.
    pub result_count: Option<i32>,
}

/// Parameters for updating or enabling an RSS subscription.
#[derive(Debug, Clone, Copy)]
pub struct IndexerRssSubscriptionParams {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Desired subscription enabled state.
    pub is_enabled: bool,
    /// Optional interval override in seconds.
    pub interval_seconds: Option<i32>,
}

/// Parameters for listing recent RSS items.
#[derive(Debug, Clone, Copy)]
pub struct IndexerRssSeenListParams {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Optional maximum row count.
    pub limit: Option<i32>,
}

/// Parameters for listing reputation rows.
#[derive(Debug, Clone, Copy)]
pub struct IndexerSourceReputationListParams<'a> {
    /// Actor performing the read.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Optional reputation window (`1h`, `24h`, `7d`).
    pub window_key: Option<&'a str>,
    /// Optional maximum row count.
    pub limit: Option<i32>,
}

/// Parameters for listing recent health events.
#[derive(Debug, Clone, Copy)]
pub struct IndexerHealthEventListParams {
    /// Actor performing the read.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Optional maximum row count.
    pub limit: Option<i32>,
}

/// Parameters for manually marking an RSS item as seen.
#[derive(Debug, Clone, Copy)]
pub struct IndexerRssSeenMarkParams<'a> {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Target instance identifier.
    pub indexer_instance_public_id: Uuid,
    /// Optional stable item identifier.
    pub item_guid: Option<&'a str>,
    /// Optional v1 infohash.
    pub infohash_v1: Option<&'a str>,
    /// Optional v2 infohash.
    pub infohash_v2: Option<&'a str>,
    /// Optional magnet hash.
    pub magnet_hash: Option<&'a str>,
}

/// Parameters for updating a health notification hook.
#[derive(Debug, Clone, Copy)]
pub struct HealthNotificationHookUpdateParams<'a> {
    /// Actor performing the update.
    pub actor_user_public_id: Uuid,
    /// Target hook identifier.
    pub hook_public_id: Uuid,
    /// Updated display name when present.
    pub display_name: Option<&'a str>,
    /// Updated threshold when present.
    pub status_threshold: Option<&'a str>,
    /// Updated webhook URL when present.
    pub webhook_url: Option<&'a str>,
    /// Updated email when present.
    pub email: Option<&'a str>,
    /// Updated enabled state when present.
    pub is_enabled: Option<bool>,
}

/// Parameters for upserting a tracker category mapping.
#[derive(Debug, Clone, Copy)]
pub struct TrackerCategoryMappingUpsertParams<'a> {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Optional app-scoped Torznab instance.
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional definition-level scope.
    pub indexer_definition_upstream_slug: Option<&'a str>,
    /// Optional instance-level scope.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category identifier.
    pub tracker_category: i32,
    /// Optional tracker subcategory identifier.
    pub tracker_subcategory: Option<i32>,
    /// Torznab category identifier.
    pub torznab_cat_id: i32,
    /// Optional media-domain scope.
    pub media_domain_key: Option<&'a str>,
}

/// Parameters for deleting a tracker category mapping.
#[derive(Debug, Clone, Copy)]
pub struct TrackerCategoryMappingDeleteParams<'a> {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Optional app-scoped Torznab instance.
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional definition-level scope.
    pub indexer_definition_upstream_slug: Option<&'a str>,
    /// Optional instance-level scope.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category identifier.
    pub tracker_category: i32,
    /// Optional tracker subcategory identifier.
    pub tracker_subcategory: Option<i32>,
}

/// Value-set item for policy rules that use `in_set`.
#[derive(Debug, Clone)]
pub struct PolicyRuleValueItem {
    /// Optional text value.
    pub value_text: Option<String>,
    /// Optional integer value.
    pub value_int: Option<i32>,
    /// Optional bigint value.
    pub value_bigint: Option<i64>,
    /// Optional UUID value.
    pub value_uuid: Option<Uuid>,
}

/// Parameters for creating a policy rule.
#[derive(Debug, Clone)]
pub struct PolicyRuleCreateParams {
    /// Actor performing the operation.
    pub actor_user_public_id: Uuid,
    /// Policy set public identifier.
    pub policy_set_public_id: Uuid,
    /// Rule type key.
    pub rule_type: String,
    /// Match field key.
    pub match_field: String,
    /// Match operator key.
    pub match_operator: String,
    /// Sort order for evaluation.
    pub sort_order: i32,
    /// Optional match text value.
    pub match_value_text: Option<String>,
    /// Optional match integer value.
    pub match_value_int: Option<i32>,
    /// Optional match UUID value.
    pub match_value_uuid: Option<Uuid>,
    /// Optional value-set items for `in_set`.
    pub value_set_items: Option<Vec<PolicyRuleValueItem>>,
    /// Action key for the rule.
    pub action: String,
    /// Severity key for the rule.
    pub severity: String,
    /// Optional case-insensitivity flag.
    pub is_case_insensitive: Option<bool>,
    /// Optional rule rationale.
    pub rationale: Option<String>,
    /// Optional expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Parameters for creating a search request.
#[derive(Debug, Clone)]
pub struct SearchRequestCreateParams<'a> {
    /// Actor performing the request (None for Torznab/system searches).
    pub actor_user_public_id: Option<Uuid>,
    /// Raw query text (may be empty for identifier-only searches).
    pub query_text: &'a str,
    /// Query type key.
    pub query_type: &'a str,
    /// Optional Torznab mode key.
    pub torznab_mode: Option<&'a str>,
    /// Optional requested media domain key.
    pub requested_media_domain_key: Option<&'a str>,
    /// Optional page size override.
    pub page_size: Option<i32>,
    /// Optional search profile public identifier.
    pub search_profile_public_id: Option<Uuid>,
    /// Optional policy set public identifier.
    pub request_policy_set_public_id: Option<Uuid>,
    /// Optional season number.
    pub season_number: Option<i32>,
    /// Optional episode number.
    pub episode_number: Option<i32>,
    /// Optional identifier types.
    pub identifier_types: Option<&'a [String]>,
    /// Optional identifier values.
    pub identifier_values: Option<&'a [String]>,
    /// Optional Torznab category ids.
    pub torznab_cat_ids: Option<&'a [i32]>,
}

/// Credentials returned for Torznab instance operations.
#[derive(Debug, Clone)]
pub struct TorznabInstanceCredentials {
    /// Torznab instance public identifier.
    pub torznab_instance_public_id: Uuid,
    /// Plaintext API key.
    pub api_key_plaintext: String,
}

/// Authenticated Torznab instance metadata.
#[derive(Debug, Clone)]
pub struct TorznabInstanceAuth {
    /// Internal Torznab instance id.
    pub torznab_instance_id: i64,
    /// Internal search profile id for this instance.
    pub search_profile_id: i64,
    /// Display name for the Torznab instance.
    pub display_name: String,
}

/// Torznab category descriptor for caps responses.
#[derive(Debug, Clone)]
pub struct TorznabCategory {
    /// Torznab category id.
    pub torznab_cat_id: i32,
    /// Human-readable category name.
    pub name: String,
}

/// Indexer-facing application operations used by the API layer.
#[async_trait]
pub trait IndexerFacade: Send + Sync {
    /// List indexer definitions from the catalog.
    async fn indexer_definition_list(
        &self,
        actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError>;
    /// Import or replace a Cardigann definition in the catalog.
    async fn indexer_definition_import_cardigann(
        &self,
        _actor_user_public_id: Uuid,
        _yaml_payload: &str,
        _is_deprecated: Option<bool>,
    ) -> Result<CardigannDefinitionImportResponse, IndexerDefinitionServiceError> {
        Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Storage,
        ))
    }
    /// Create a new tag and return its public identifier.
    async fn tag_create(
        &self,
        actor_user_public_id: Uuid,
        tag_key: &str,
        display_name: &str,
    ) -> Result<Uuid, TagServiceError>;
    /// List active tags for operator workflows.
    async fn tag_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<TagListItemResponse>, TagServiceError> {
        Err(TagServiceError::new(TagServiceErrorKind::Storage))
    }
    /// Update a tag display name and return its public identifier.
    async fn tag_update(
        &self,
        actor_user_public_id: Uuid,
        tag_public_id: Option<Uuid>,
        tag_key: Option<&str>,
        display_name: &str,
    ) -> Result<Uuid, TagServiceError>;
    /// Soft delete a tag.
    async fn tag_delete(
        &self,
        actor_user_public_id: Uuid,
        tag_public_id: Option<Uuid>,
        tag_key: Option<&str>,
    ) -> Result<(), TagServiceError>;
    /// List operator-visible secret metadata.
    async fn secret_metadata_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<SecretMetadataResponse>, SecretServiceError> {
        Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
    }
    /// List configured health notification hooks.
    async fn indexer_health_notification_hook_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerHealthNotificationHookResponse>, HealthNotificationServiceError> {
        Err(HealthNotificationServiceError::new(
            HealthNotificationServiceErrorKind::Storage,
        ))
    }
    /// Fetch a single configured health notification hook.
    async fn indexer_health_notification_hook_get(
        &self,
        _actor_user_public_id: Uuid,
        _hook_public_id: Uuid,
    ) -> Result<IndexerHealthNotificationHookResponse, HealthNotificationServiceError> {
        Err(HealthNotificationServiceError::new(
            HealthNotificationServiceErrorKind::Storage,
        ))
    }
    /// Create a health notification hook and return its public identifier.
    async fn indexer_health_notification_hook_create(
        &self,
        _actor_user_public_id: Uuid,
        _channel: &str,
        _display_name: &str,
        _status_threshold: &str,
        _webhook_url: Option<&str>,
        _email: Option<&str>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        Err(HealthNotificationServiceError::new(
            HealthNotificationServiceErrorKind::Storage,
        ))
    }
    /// Update a health notification hook and return its public identifier.
    async fn indexer_health_notification_hook_update(
        &self,
        _params: HealthNotificationHookUpdateParams<'_>,
    ) -> Result<Uuid, HealthNotificationServiceError> {
        Err(HealthNotificationServiceError::new(
            HealthNotificationServiceErrorKind::Storage,
        ))
    }
    /// Delete a health notification hook.
    async fn indexer_health_notification_hook_delete(
        &self,
        _actor_user_public_id: Uuid,
        _hook_public_id: Uuid,
    ) -> Result<(), HealthNotificationServiceError> {
        Err(HealthNotificationServiceError::new(
            HealthNotificationServiceErrorKind::Storage,
        ))
    }
    /// Create a new search request and return its identifiers.
    async fn search_request_create(
        &self,
        _params: SearchRequestCreateParams<'_>,
    ) -> Result<SearchRequestCreateResponse, SearchRequestServiceError> {
        Err(SearchRequestServiceError::new(
            SearchRequestServiceErrorKind::Storage,
        ))
    }
    /// Cancel a search request.
    async fn search_request_cancel(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
    ) -> Result<(), SearchRequestServiceError> {
        Err(SearchRequestServiceError::new(
            SearchRequestServiceErrorKind::Storage,
        ))
    }
    /// List available pages for a search request.
    async fn search_page_list(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
    ) -> Result<SearchPageListResponse, SearchRequestServiceError> {
        Err(SearchRequestServiceError::new(
            SearchRequestServiceErrorKind::Storage,
        ))
    }
    /// Fetch the items for a specific search request page.
    async fn search_page_fetch(
        &self,
        _actor_user_public_id: Uuid,
        _search_request_public_id: Uuid,
        _page_number: i32,
    ) -> Result<SearchPageResponse, SearchRequestServiceError> {
        Err(SearchRequestServiceError::new(
            SearchRequestServiceErrorKind::Storage,
        ))
    }
    /// Create a new routing policy and return its public identifier.
    async fn routing_policy_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        mode: &str,
    ) -> Result<Uuid, RoutingPolicyServiceError>;
    /// Set a routing policy parameter.
    async fn routing_policy_set_param(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        value_plain: Option<&str>,
        value_int: Option<i32>,
        value_bool: Option<bool>,
    ) -> Result<(), RoutingPolicyServiceError>;
    /// Bind a secret to a routing policy parameter.
    async fn routing_policy_bind_secret(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        param_key: &str,
        secret_public_id: Uuid,
    ) -> Result<(), RoutingPolicyServiceError>;
    /// Fetch a routing policy with visible parameter and rate-limit assignments.
    async fn routing_policy_get(
        &self,
        _actor_user_public_id: Uuid,
        _routing_policy_public_id: Uuid,
    ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }
    /// List routing policies for operator inventory flows.
    async fn routing_policy_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<RoutingPolicyListItemResponse>, RoutingPolicyServiceError> {
        Err(RoutingPolicyServiceError::new(
            RoutingPolicyServiceErrorKind::Storage,
        ))
    }
    /// Create a new rate limit policy and return its public identifier.
    async fn rate_limit_policy_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        rpm: i32,
        burst: i32,
        concurrent: i32,
    ) -> Result<Uuid, RateLimitPolicyServiceError>;
    /// Update an existing rate limit policy.
    async fn rate_limit_policy_update(
        &self,
        actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
        display_name: Option<&str>,
        rpm: Option<i32>,
        burst: Option<i32>,
        concurrent: Option<i32>,
    ) -> Result<(), RateLimitPolicyServiceError>;
    /// Soft delete a rate limit policy.
    async fn rate_limit_policy_soft_delete(
        &self,
        actor_user_public_id: Uuid,
        rate_limit_policy_public_id: Uuid,
    ) -> Result<(), RateLimitPolicyServiceError>;
    /// Assign a rate limit policy to an indexer instance.
    async fn indexer_instance_set_rate_limit_policy(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError>;
    /// Assign a rate limit policy to a routing policy.
    async fn routing_policy_set_rate_limit_policy(
        &self,
        actor_user_public_id: Uuid,
        routing_policy_public_id: Uuid,
        rate_limit_policy_public_id: Option<Uuid>,
    ) -> Result<(), RateLimitPolicyServiceError>;
    /// List rate-limit policies for operator inventory flows.
    async fn rate_limit_policy_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<RateLimitPolicyListItemResponse>, RateLimitPolicyServiceError> {
        Err(RateLimitPolicyServiceError::new(
            RateLimitPolicyServiceErrorKind::Storage,
        ))
    }
    /// Create a search profile and return its public identifier.
    async fn search_profile_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        is_default: Option<bool>,
        page_size: Option<i32>,
        default_media_domain_key: Option<&str>,
        user_public_id: Option<Uuid>,
    ) -> Result<Uuid, SearchProfileServiceError>;
    /// Update a search profile and return its public identifier.
    async fn search_profile_update(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: Option<&str>,
        page_size: Option<i32>,
    ) -> Result<Uuid, SearchProfileServiceError>;
    /// Set the default search profile.
    async fn search_profile_set_default(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        page_size: Option<i32>,
    ) -> Result<(), SearchProfileServiceError>;
    /// Set the default media domain for a search profile.
    async fn search_profile_set_default_domain(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        default_media_domain_key: Option<&str>,
    ) -> Result<(), SearchProfileServiceError>;
    /// Replace the domain allowlist for a search profile.
    async fn search_profile_set_domain_allowlist(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        media_domain_keys: &[String],
    ) -> Result<(), SearchProfileServiceError>;
    /// Add a policy set to a search profile.
    async fn search_profile_add_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError>;
    /// Remove a policy set from a search profile.
    async fn search_profile_remove_policy_set(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), SearchProfileServiceError>;
    /// Allow indexer instances for a search profile.
    async fn search_profile_indexer_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError>;
    /// Block indexer instances for a search profile.
    async fn search_profile_indexer_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        indexer_instance_public_ids: &[Uuid],
    ) -> Result<(), SearchProfileServiceError>;
    /// Allow tags for a search profile.
    async fn search_profile_tag_allow(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError>;
    /// Block tags for a search profile.
    async fn search_profile_tag_block(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError>;
    /// Prefer tags for a search profile.
    async fn search_profile_tag_prefer(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), SearchProfileServiceError>;
    /// Create an import job and return its public identifier.
    async fn import_job_create(
        &self,
        actor_user_public_id: Uuid,
        source: &str,
        is_dry_run: Option<bool>,
        target_search_profile_public_id: Option<Uuid>,
        target_torznab_instance_public_id: Option<Uuid>,
    ) -> Result<Uuid, ImportJobServiceError> {
        let _ = (
            actor_user_public_id,
            source,
            is_dry_run,
            target_search_profile_public_id,
            target_torznab_instance_public_id,
        );
        Err(ImportJobServiceError::new(
            ImportJobServiceErrorKind::Storage,
        ))
    }
    /// Start an import job using the Prowlarr API path.
    async fn import_job_run_prowlarr_api(
        &self,
        import_job_public_id: Uuid,
        prowlarr_url: &str,
        prowlarr_api_key_secret_public_id: Uuid,
    ) -> Result<(), ImportJobServiceError> {
        let _ = (
            import_job_public_id,
            prowlarr_url,
            prowlarr_api_key_secret_public_id,
        );
        Err(ImportJobServiceError::new(
            ImportJobServiceErrorKind::Storage,
        ))
    }
    /// Start an import job using a Prowlarr backup.
    async fn import_job_run_prowlarr_backup(
        &self,
        import_job_public_id: Uuid,
        backup_blob_ref: &str,
    ) -> Result<(), ImportJobServiceError> {
        let _ = (import_job_public_id, backup_blob_ref);
        Err(ImportJobServiceError::new(
            ImportJobServiceErrorKind::Storage,
        ))
    }
    /// Get status for an import job.
    async fn import_job_get_status(
        &self,
        import_job_public_id: Uuid,
    ) -> Result<ImportJobStatusResponse, ImportJobServiceError> {
        let _ = import_job_public_id;
        Err(ImportJobServiceError::new(
            ImportJobServiceErrorKind::Storage,
        ))
    }
    /// List results for an import job.
    async fn import_job_list_results(
        &self,
        import_job_public_id: Uuid,
    ) -> Result<Vec<ImportJobResultResponse>, ImportJobServiceError> {
        let _ = import_job_public_id;
        Err(ImportJobServiceError::new(
            ImportJobServiceErrorKind::Storage,
        ))
    }
    /// List source metadata conflicts for operator review.
    async fn source_metadata_conflict_list(
        &self,
        actor_user_public_id: Uuid,
        include_resolved: Option<bool>,
        limit: Option<i32>,
    ) -> Result<Vec<IndexerSourceMetadataConflictResponse>, SourceMetadataConflictServiceError>
    {
        let _ = (actor_user_public_id, include_resolved, limit);
        Err(SourceMetadataConflictServiceError::new(
            SourceMetadataConflictServiceErrorKind::Storage,
        ))
    }
    /// Resolve a source metadata conflict.
    async fn source_metadata_conflict_resolve(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution: &str,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        let _ = (
            actor_user_public_id,
            conflict_id,
            resolution,
            resolution_note,
        );
        Err(SourceMetadataConflictServiceError::new(
            SourceMetadataConflictServiceErrorKind::Storage,
        ))
    }
    /// Reopen a resolved source metadata conflict.
    async fn source_metadata_conflict_reopen(
        &self,
        actor_user_public_id: Uuid,
        conflict_id: i64,
        resolution_note: Option<&str>,
    ) -> Result<(), SourceMetadataConflictServiceError> {
        let _ = (actor_user_public_id, conflict_id, resolution_note);
        Err(SourceMetadataConflictServiceError::new(
            SourceMetadataConflictServiceErrorKind::Storage,
        ))
    }
    /// Export a sanitized indexer backup snapshot.
    async fn indexer_backup_export(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<IndexerBackupExportResponse, IndexerBackupServiceError> {
        Err(IndexerBackupServiceError::new(
            IndexerBackupServiceErrorKind::Storage,
        ))
    }
    /// Restore an indexer backup snapshot.
    async fn indexer_backup_restore(
        &self,
        _actor_user_public_id: Uuid,
        _snapshot: &crate::models::IndexerBackupSnapshot,
    ) -> Result<IndexerBackupRestoreResponse, IndexerBackupServiceError> {
        Err(IndexerBackupServiceError::new(
            IndexerBackupServiceErrorKind::Storage,
        ))
    }
    /// Create a policy set and return its public identifier.
    async fn policy_set_create(
        &self,
        actor_user_public_id: Uuid,
        display_name: &str,
        scope: &str,
        enabled: Option<bool>,
    ) -> Result<Uuid, PolicyServiceError> {
        let _ = (actor_user_public_id, display_name, scope, enabled);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Update a policy set and return its public identifier.
    async fn policy_set_update(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        display_name: Option<&str>,
    ) -> Result<Uuid, PolicyServiceError> {
        let _ = (actor_user_public_id, policy_set_public_id, display_name);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Enable a policy set.
    async fn policy_set_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let _ = (actor_user_public_id, policy_set_public_id);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Disable a policy set.
    async fn policy_set_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let _ = (actor_user_public_id, policy_set_public_id);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Reorder policy sets by public identifier.
    async fn policy_set_reorder(
        &self,
        actor_user_public_id: Uuid,
        ordered_policy_set_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        let _ = (actor_user_public_id, ordered_policy_set_public_ids);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Create a policy rule and return its public identifier.
    async fn policy_rule_create(
        &self,
        params: PolicyRuleCreateParams,
    ) -> Result<Uuid, PolicyServiceError> {
        let _ = params;
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Enable a policy rule.
    async fn policy_rule_enable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let _ = (actor_user_public_id, policy_rule_public_id);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Disable a policy rule.
    async fn policy_rule_disable(
        &self,
        actor_user_public_id: Uuid,
        policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        let _ = (actor_user_public_id, policy_rule_public_id);
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Reorder policy rules within a policy set.
    async fn policy_rule_reorder(
        &self,
        actor_user_public_id: Uuid,
        policy_set_public_id: Uuid,
        ordered_policy_rule_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        let _ = (
            actor_user_public_id,
            policy_set_public_id,
            ordered_policy_rule_public_ids,
        );
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }
    /// Upsert a tracker category mapping.
    async fn tracker_category_mapping_upsert(
        &self,
        params: TrackerCategoryMappingUpsertParams<'_>,
    ) -> Result<(), CategoryMappingServiceError>;
    /// Delete a tracker category mapping.
    async fn tracker_category_mapping_delete(
        &self,
        params: TrackerCategoryMappingDeleteParams<'_>,
    ) -> Result<(), CategoryMappingServiceError>;
    /// Upsert a media domain to torznab category mapping.
    async fn media_domain_mapping_upsert(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
        is_primary: Option<bool>,
    ) -> Result<(), CategoryMappingServiceError>;
    /// Delete a media domain to torznab category mapping.
    async fn media_domain_mapping_delete(
        &self,
        actor_user_public_id: Uuid,
        media_domain_key: &str,
        torznab_cat_id: i32,
    ) -> Result<(), CategoryMappingServiceError>;
    /// Create a Torznab instance and return its credentials.
    async fn torznab_instance_create(
        &self,
        actor_user_public_id: Uuid,
        search_profile_public_id: Uuid,
        display_name: &str,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError>;
    /// Rotate a Torznab instance API key and return the new key.
    async fn torznab_instance_rotate_key(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<TorznabInstanceCredentials, TorznabInstanceServiceError>;
    /// Enable or disable a Torznab instance.
    async fn torznab_instance_enable_disable(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
        is_enabled: bool,
    ) -> Result<(), TorznabInstanceServiceError>;
    /// Soft delete a Torznab instance.
    async fn torznab_instance_soft_delete(
        &self,
        actor_user_public_id: Uuid,
        torznab_instance_public_id: Uuid,
    ) -> Result<(), TorznabInstanceServiceError>;
    /// Authenticate a Torznab API key and return instance metadata.
    async fn torznab_instance_authenticate(
        &self,
        torznab_instance_public_id: Uuid,
        api_key_plaintext: &str,
    ) -> Result<TorznabInstanceAuth, TorznabAccessError> {
        let _ = (torznab_instance_public_id, api_key_plaintext);
        Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage))
    }
    /// Prepare a Torznab download redirect and record an acquisition attempt.
    async fn torznab_download_prepare(
        &self,
        torznab_instance_public_id: Uuid,
        canonical_torrent_source_public_id: Uuid,
    ) -> Result<Option<String>, TorznabAccessError> {
        let _ = (
            torznab_instance_public_id,
            canonical_torrent_source_public_id,
        );
        Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage))
    }
    /// List Torznab categories for caps responses.
    async fn torznab_category_list(&self) -> Result<Vec<TorznabCategory>, TorznabAccessError> {
        Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage))
    }
    /// Resolve emitted Torznab category ids for a feed item in a downstream app context.
    async fn torznab_feed_category_ids(
        &self,
        torznab_instance_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        tracker_category: Option<i32>,
        tracker_subcategory: Option<i32>,
    ) -> Result<Vec<i32>, TorznabAccessError> {
        let _ = (
            torznab_instance_public_id,
            indexer_instance_public_id,
            tracker_category,
            tracker_subcategory,
        );
        Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage))
    }
    /// Create a new indexer instance.
    async fn indexer_instance_create(
        &self,
        actor_user_public_id: Uuid,
        indexer_definition_upstream_slug: &str,
        display_name: &str,
        priority: Option<i32>,
        trust_tier_key: Option<&str>,
        routing_policy_public_id: Option<Uuid>,
    ) -> Result<Uuid, IndexerInstanceServiceError>;
    /// Update an existing indexer instance.
    async fn indexer_instance_update(
        &self,
        params: IndexerInstanceUpdateParams<'_>,
    ) -> Result<Uuid, IndexerInstanceServiceError>;
    /// List indexer instances for operator inventory flows.
    async fn indexer_instance_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerInstanceListItemResponse>, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// Replace media domain assignments for an indexer instance.
    async fn indexer_instance_set_media_domains(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        media_domain_keys: &[String],
    ) -> Result<(), IndexerInstanceServiceError>;
    /// Replace tag assignments for an indexer instance.
    async fn indexer_instance_set_tags(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        tag_public_ids: Option<&[Uuid]>,
        tag_keys: Option<&[String]>,
    ) -> Result<(), IndexerInstanceServiceError>;
    /// Set a field value on an indexer instance.
    async fn indexer_instance_field_set_value(
        &self,
        params: IndexerInstanceFieldValueParams<'_>,
    ) -> Result<(), IndexerInstanceFieldError>;
    /// Bind a secret to an indexer instance field.
    async fn indexer_instance_field_bind_secret(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
        field_name: &str,
        secret_public_id: Uuid,
    ) -> Result<(), IndexerInstanceFieldError>;
    /// Reset Cloudflare mitigation state for an indexer instance.
    async fn indexer_cf_state_reset(
        &self,
        params: IndexerCfStateResetParams<'_>,
    ) -> Result<(), IndexerInstanceServiceError>;
    /// Fetch Cloudflare mitigation state for an indexer instance.
    async fn indexer_cf_state_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerCfStateResponse, IndexerInstanceServiceError>;
    /// Fetch the derived connectivity profile for an indexer instance.
    async fn indexer_connectivity_profile_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerConnectivityProfileResponse, IndexerInstanceServiceError> {
        let _ = (actor_user_public_id, indexer_instance_public_id);
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// List recent source reputation snapshots for an indexer instance.
    async fn indexer_source_reputation_list(
        &self,
        params: IndexerSourceReputationListParams<'_>,
    ) -> Result<Vec<IndexerSourceReputationResponse>, IndexerInstanceServiceError> {
        let _ = params;
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// List recent health events for an indexer instance.
    async fn indexer_health_event_list(
        &self,
        params: IndexerHealthEventListParams,
    ) -> Result<Vec<IndexerHealthEventResponse>, IndexerInstanceServiceError> {
        let _ = params;
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// Prepare an indexer instance test for the executor.
    async fn indexer_instance_test_prepare(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerInstanceTestPrepareResponse, IndexerInstanceServiceError>;
    /// Finalize an indexer instance test result.
    async fn indexer_instance_test_finalize(
        &self,
        params: IndexerInstanceTestFinalizeParams<'_>,
    ) -> Result<IndexerInstanceTestFinalizeResponse, IndexerInstanceServiceError>;
    /// Fetch RSS subscription status for an indexer instance.
    async fn indexer_rss_subscription_get(
        &self,
        actor_user_public_id: Uuid,
        indexer_instance_public_id: Uuid,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let _ = (actor_user_public_id, indexer_instance_public_id);
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// Enable or update RSS subscription settings for an indexer instance.
    async fn indexer_rss_subscription_set(
        &self,
        params: IndexerRssSubscriptionParams,
    ) -> Result<IndexerRssSubscriptionResponse, IndexerInstanceServiceError> {
        let _ = params;
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// List recent RSS items for an indexer instance.
    async fn indexer_rss_seen_list(
        &self,
        params: IndexerRssSeenListParams,
    ) -> Result<Vec<IndexerRssSeenItemResponse>, IndexerInstanceServiceError> {
        let _ = params;
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// Manually mark an RSS item as seen for an indexer instance.
    async fn indexer_rss_seen_mark(
        &self,
        params: IndexerRssSeenMarkParams<'_>,
    ) -> Result<IndexerRssSeenMarkResponse, IndexerInstanceServiceError> {
        let _ = params;
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }
    /// Create a new secret and return its public identifier.
    async fn secret_create(
        &self,
        actor_user_public_id: Uuid,
        secret_type: &str,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError>;
    /// Rotate an existing secret.
    async fn secret_rotate(
        &self,
        actor_user_public_id: Uuid,
        secret_public_id: Uuid,
        secret_value: &str,
    ) -> Result<Uuid, SecretServiceError>;
    /// Revoke a secret.
    async fn secret_revoke(
        &self,
        actor_user_public_id: Uuid,
        secret_public_id: Uuid,
    ) -> Result<(), SecretServiceError>;
}

/// Classification for indexer definition service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerDefinitionServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by indexer definition service operations.
#[derive(Debug, Clone)]
pub struct IndexerDefinitionServiceError {
    kind: IndexerDefinitionServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl IndexerDefinitionServiceError {
    /// Create a new indexer definition service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: IndexerDefinitionServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> IndexerDefinitionServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for IndexerDefinitionServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("indexer definition service error")
    }
}

impl Error for IndexerDefinitionServiceError {}

/// Classification for indexer backup service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerBackupServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Referenced resource was not found.
    NotFound,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by indexer backup service operations.
#[derive(Debug, Clone)]
pub struct IndexerBackupServiceError {
    kind: IndexerBackupServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl IndexerBackupServiceError {
    /// Create a new indexer backup service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: IndexerBackupServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> IndexerBackupServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for IndexerBackupServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("indexer backup service error")
    }
}

impl Error for IndexerBackupServiceError {}

/// Classification for tag service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by tag service operations.
#[derive(Debug, Clone)]
pub struct TagServiceError {
    kind: TagServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl TagServiceError {
    /// Create a new tag service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: TagServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> TagServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for TagServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("tag service error")
    }
}

impl Error for TagServiceError {}

/// Classification for health notification hook service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthNotificationServiceErrorKind {
    /// Input validation failed.
    Invalid,
    /// Requested hook does not exist.
    NotFound,
    /// Operation is not authorized for the supplied actor.
    Unauthorized,
    /// Storage or unexpected failure.
    Storage,
}

/// Error returned by health notification hook service operations.
#[derive(Debug, Clone)]
pub struct HealthNotificationServiceError {
    kind: HealthNotificationServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl HealthNotificationServiceError {
    /// Create a new health notification hook service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: HealthNotificationServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database detail code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for client context.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the failure classification.
    #[must_use]
    pub const fn kind(&self) -> HealthNotificationServiceErrorKind {
        self.kind
    }

    /// Database detail code when available.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// SQLSTATE when available.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for HealthNotificationServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "health notification hook service error")
    }
}

impl Error for HealthNotificationServiceError {}

/// Classification for routing policy service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingPolicyServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by routing policy service operations.
#[derive(Debug, Clone)]
pub struct RoutingPolicyServiceError {
    kind: RoutingPolicyServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl RoutingPolicyServiceError {
    /// Create a new routing policy service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: RoutingPolicyServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> RoutingPolicyServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for RoutingPolicyServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("routing policy service error")
    }
}

impl Error for RoutingPolicyServiceError {}

/// Classification for rate limit policy service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitPolicyServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by rate limit policy service operations.
#[derive(Debug, Clone)]
pub struct RateLimitPolicyServiceError {
    kind: RateLimitPolicyServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl RateLimitPolicyServiceError {
    /// Create a new rate limit policy service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: RateLimitPolicyServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> RateLimitPolicyServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for RateLimitPolicyServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("rate limit policy service error")
    }
}

impl Error for RateLimitPolicyServiceError {}

/// Classification for search profile service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchProfileServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by search profile service operations.
#[derive(Debug, Clone)]
pub struct SearchProfileServiceError {
    kind: SearchProfileServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl SearchProfileServiceError {
    /// Create a new search profile service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: SearchProfileServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> SearchProfileServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for SearchProfileServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("search profile service error")
    }
}

impl Error for SearchProfileServiceError {}

/// Classification for search request service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRequestServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by search request service operations.
#[derive(Debug, Clone)]
pub struct SearchRequestServiceError {
    kind: SearchRequestServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl SearchRequestServiceError {
    /// Create a new search request service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: SearchRequestServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> SearchRequestServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for SearchRequestServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("search request service error")
    }
}

impl Error for SearchRequestServiceError {}

/// Classification for import job service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportJobServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by import job service operations.
#[derive(Debug, Clone)]
pub struct ImportJobServiceError {
    kind: ImportJobServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl ImportJobServiceError {
    /// Create a new import job service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: ImportJobServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> ImportJobServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for ImportJobServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("import job service error")
    }
}

impl Error for ImportJobServiceError {}

/// Classification for source metadata conflict service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMetadataConflictServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by source metadata conflict service operations.
#[derive(Debug, Clone)]
pub struct SourceMetadataConflictServiceError {
    kind: SourceMetadataConflictServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl SourceMetadataConflictServiceError {
    /// Create a new source metadata conflict service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: SourceMetadataConflictServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> SourceMetadataConflictServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for SourceMetadataConflictServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("source metadata conflict service error")
    }
}

impl Error for SourceMetadataConflictServiceError {}

/// Classification for policy service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by policy service operations.
#[derive(Debug, Clone)]
pub struct PolicyServiceError {
    kind: PolicyServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl PolicyServiceError {
    /// Create a new policy service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: PolicyServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> PolicyServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for PolicyServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("policy service error")
    }
}

impl Error for PolicyServiceError {}

/// Classification for category mapping service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryMappingServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by category mapping service operations.
#[derive(Debug, Clone)]
pub struct CategoryMappingServiceError {
    kind: CategoryMappingServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl CategoryMappingServiceError {
    /// Create a new category mapping service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: CategoryMappingServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> CategoryMappingServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for CategoryMappingServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("category mapping service error")
    }
}

impl Error for CategoryMappingServiceError {}

/// Classification for Torznab instance service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorznabInstanceServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by Torznab instance service operations.
#[derive(Debug, Clone)]
pub struct TorznabInstanceServiceError {
    kind: TorznabInstanceServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl TorznabInstanceServiceError {
    /// Create a new Torznab instance service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: TorznabInstanceServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> TorznabInstanceServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for TorznabInstanceServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("torznab instance service error")
    }
}

impl Error for TorznabInstanceServiceError {}

/// Classification for Torznab access failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorznabAccessErrorKind {
    /// API key is missing or invalid.
    Unauthorized,
    /// Target instance is missing, disabled, or deleted.
    NotFound,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by Torznab access operations.
#[derive(Debug, Clone)]
pub struct TorznabAccessError {
    kind: TorznabAccessErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl TorznabAccessError {
    /// Create a new Torznab access error with the supplied kind.
    #[must_use]
    pub const fn new(kind: TorznabAccessErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> TorznabAccessErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for TorznabAccessError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("torznab access error")
    }
}

impl Error for TorznabAccessError {}

/// Classification for indexer instance service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerInstanceServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by indexer instance service operations.
#[derive(Debug, Clone)]
pub struct IndexerInstanceServiceError {
    kind: IndexerInstanceServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl IndexerInstanceServiceError {
    /// Create a new indexer instance service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: IndexerInstanceServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> IndexerInstanceServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for IndexerInstanceServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("indexer instance service error")
    }
}

impl Error for IndexerInstanceServiceError {}

/// Classification for indexer instance field service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexerInstanceFieldErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Operation conflicted with existing state.
    Conflict,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by indexer instance field operations.
#[derive(Debug, Clone)]
pub struct IndexerInstanceFieldError {
    kind: IndexerInstanceFieldErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl IndexerInstanceFieldError {
    /// Create a new indexer instance field error with the supplied kind.
    #[must_use]
    pub const fn new(kind: IndexerInstanceFieldErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> IndexerInstanceFieldErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for IndexerInstanceFieldError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("indexer instance field service error")
    }
}

impl Error for IndexerInstanceFieldError {}

/// Classification for secret service failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretServiceErrorKind {
    /// Input failed validation rules.
    Invalid,
    /// Target resource was not found.
    NotFound,
    /// Actor identity is missing or unauthorized.
    Unauthorized,
    /// Storage or unexpected failures.
    Storage,
}

/// Error returned by secret service operations.
#[derive(Debug, Clone)]
pub struct SecretServiceError {
    kind: SecretServiceErrorKind,
    code: Option<String>,
    sqlstate: Option<String>,
}

impl SecretServiceError {
    /// Create a new secret service error with the supplied kind.
    #[must_use]
    pub const fn new(kind: SecretServiceErrorKind) -> Self {
        Self {
            kind,
            code: None,
            sqlstate: None,
        }
    }

    /// Attach a database error code for client context.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a SQLSTATE value for diagnostics.
    #[must_use]
    pub fn with_sqlstate(mut self, sqlstate: impl Into<String>) -> Self {
        self.sqlstate = Some(sqlstate.into());
        self
    }

    /// Return the error kind.
    #[must_use]
    pub const fn kind(&self) -> SecretServiceErrorKind {
        self.kind
    }

    /// Return the database detail code, when present.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    /// Return the SQLSTATE, when present.
    #[must_use]
    pub fn sqlstate(&self) -> Option<&str> {
        self.sqlstate.as_deref()
    }
}

impl Display for SecretServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("secret service error")
    }
}

impl Error for SecretServiceError {}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct NoopIndexers;

#[cfg(test)]
#[async_trait]
impl IndexerFacade for NoopIndexers {
    async fn indexer_definition_list(
        &self,
        _actor_user_public_id: Uuid,
    ) -> Result<Vec<IndexerDefinitionResponse>, IndexerDefinitionServiceError> {
        Err(IndexerDefinitionServiceError::new(
            IndexerDefinitionServiceErrorKind::Storage,
        ))
    }

    async fn tag_create(
        &self,
        _actor_user_public_id: Uuid,
        _tag_key: &str,
        _display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        Err(TagServiceError::new(TagServiceErrorKind::Storage))
    }

    async fn tag_update(
        &self,
        _actor_user_public_id: Uuid,
        _tag_public_id: Option<Uuid>,
        _tag_key: Option<&str>,
        _display_name: &str,
    ) -> Result<Uuid, TagServiceError> {
        Err(TagServiceError::new(TagServiceErrorKind::Storage))
    }

    async fn tag_delete(
        &self,
        _actor_user_public_id: Uuid,
        _tag_public_id: Option<Uuid>,
        _tag_key: Option<&str>,
    ) -> Result<(), TagServiceError> {
        Err(TagServiceError::new(TagServiceErrorKind::Storage))
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

    async fn routing_policy_get(
        &self,
        _actor_user_public_id: Uuid,
        _routing_policy_public_id: Uuid,
    ) -> Result<RoutingPolicyDetailResponse, RoutingPolicyServiceError> {
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

    async fn policy_set_create(
        &self,
        _actor_user_public_id: Uuid,
        _display_name: &str,
        _scope: &str,
        _enabled: Option<bool>,
    ) -> Result<Uuid, PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_set_update(
        &self,
        _actor_user_public_id: Uuid,
        _policy_set_public_id: Uuid,
        _display_name: Option<&str>,
    ) -> Result<Uuid, PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_set_enable(
        &self,
        _actor_user_public_id: Uuid,
        _policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_set_disable(
        &self,
        _actor_user_public_id: Uuid,
        _policy_set_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_set_reorder(
        &self,
        _actor_user_public_id: Uuid,
        _ordered_policy_set_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_rule_create(
        &self,
        _params: PolicyRuleCreateParams,
    ) -> Result<Uuid, PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_rule_enable(
        &self,
        _actor_user_public_id: Uuid,
        _policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_rule_disable(
        &self,
        _actor_user_public_id: Uuid,
        _policy_rule_public_id: Uuid,
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn policy_rule_reorder(
        &self,
        _actor_user_public_id: Uuid,
        _policy_set_public_id: Uuid,
        _ordered_policy_rule_public_ids: &[Uuid],
    ) -> Result<(), PolicyServiceError> {
        Err(PolicyServiceError::new(PolicyServiceErrorKind::Storage))
    }

    async fn tracker_category_mapping_upsert(
        &self,
        _params: TrackerCategoryMappingUpsertParams<'_>,
    ) -> Result<(), CategoryMappingServiceError> {
        Err(CategoryMappingServiceError::new(
            CategoryMappingServiceErrorKind::Storage,
        ))
    }

    async fn tracker_category_mapping_delete(
        &self,
        _params: TrackerCategoryMappingDeleteParams<'_>,
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

    async fn indexer_instance_create(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_definition_upstream_slug: &str,
        _display_name: &str,
        _priority: Option<i32>,
        _trust_tier_key: Option<&str>,
        _routing_policy_public_id: Option<Uuid>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_update(
        &self,
        _params: IndexerInstanceUpdateParams<'_>,
    ) -> Result<Uuid, IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_set_media_domains(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _media_domain_keys: &[String],
    ) -> Result<(), IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_set_tags(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _tag_public_ids: Option<&[Uuid]>,
        _tag_keys: Option<&[String]>,
    ) -> Result<(), IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
    }

    async fn indexer_instance_field_set_value(
        &self,
        _params: IndexerInstanceFieldValueParams<'_>,
    ) -> Result<(), IndexerInstanceFieldError> {
        Err(IndexerInstanceFieldError::new(
            IndexerInstanceFieldErrorKind::Storage,
        ))
    }

    async fn indexer_instance_field_bind_secret(
        &self,
        _actor_user_public_id: Uuid,
        _indexer_instance_public_id: Uuid,
        _field_name: &str,
        _secret_public_id: Uuid,
    ) -> Result<(), IndexerInstanceFieldError> {
        Err(IndexerInstanceFieldError::new(
            IndexerInstanceFieldErrorKind::Storage,
        ))
    }

    async fn indexer_cf_state_reset(
        &self,
        _params: IndexerCfStateResetParams<'_>,
    ) -> Result<(), IndexerInstanceServiceError> {
        Err(IndexerInstanceServiceError::new(
            IndexerInstanceServiceErrorKind::Storage,
        ))
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

    async fn secret_create(
        &self,
        _actor_user_public_id: Uuid,
        _secret_type: &str,
        _secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
    }

    async fn secret_rotate(
        &self,
        _actor_user_public_id: Uuid,
        _secret_public_id: Uuid,
        _secret_value: &str,
    ) -> Result<Uuid, SecretServiceError> {
        Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
    }

    async fn secret_revoke(
        &self,
        _actor_user_public_id: Uuid,
        _secret_public_id: Uuid,
    ) -> Result<(), SecretServiceError> {
        Err(SecretServiceError::new(SecretServiceErrorKind::Storage))
    }
}

#[cfg(test)]
pub(crate) fn test_indexers() -> std::sync::Arc<dyn IndexerFacade> {
    std::sync::Arc::new(NoopIndexers)
}
