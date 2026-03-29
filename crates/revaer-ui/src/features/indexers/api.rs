//! API helpers for indexer admin actions.
//!
//! # Design
//! - Wrap the shared authenticated API client with operation-specific helpers.
//! - Keep request construction close to the admin feature to avoid leaking raw form strings.
//! - Return typed responses for rendering and JSON logging.

use crate::features::indexers::logic::{
    optional_bool, optional_i32, optional_string, optional_uuid, required_i32, required_i64,
    required_uuid, string_list, uuid_list,
};
use crate::features::indexers::state::{AppSyncProvisionSummary, IndexersDraft};
use crate::models::{
    CardigannDefinitionImportRequest, CardigannDefinitionImportResponse, ImportJobCreateRequest,
    ImportJobResponse, ImportJobResultsResponse, ImportJobRunProwlarrApiRequest,
    ImportJobRunProwlarrBackupRequest, ImportJobStatusResponse, IndexerBackupExportResponse,
    IndexerBackupRestoreRequest, IndexerBackupRestoreResponse, IndexerBackupSnapshot,
    IndexerCfStateResetRequest, IndexerCfStateResponse, IndexerConnectivityProfileResponse,
    IndexerDefinitionListResponse, IndexerHealthEventListResponse,
    IndexerHealthNotificationHookCreateRequest, IndexerHealthNotificationHookDeleteRequest,
    IndexerHealthNotificationHookListResponse, IndexerHealthNotificationHookResponse,
    IndexerHealthNotificationHookUpdateRequest, IndexerInstanceCreateRequest,
    IndexerInstanceFieldSecretBindRequest, IndexerInstanceFieldValueRequest,
    IndexerInstanceMediaDomainsRequest, IndexerInstanceResponse, IndexerInstanceTagsRequest,
    IndexerInstanceTestFinalizeRequest, IndexerInstanceTestFinalizeResponse,
    IndexerInstanceTestPrepareResponse, IndexerInstanceUpdateRequest, IndexerRssSeenItemsResponse,
    IndexerRssSeenMarkRequest, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerRssSubscriptionUpdateRequest, IndexerSourceMetadataConflictListResponse,
    IndexerSourceMetadataConflictReopenRequest, IndexerSourceMetadataConflictResolveRequest,
    IndexerSourceReputationListResponse, PolicyRuleCreateRequest, PolicyRuleResponse,
    PolicyRuleValueItemRequest, PolicySetCreateRequest, PolicySetResponse,
    RateLimitPolicyAssignmentRequest, RateLimitPolicyCreateRequest, RateLimitPolicyResponse,
    RateLimitPolicyUpdateRequest, RoutingPolicyCreateRequest, RoutingPolicyDetailResponse,
    RoutingPolicyParamSetRequest, RoutingPolicyResponse, RoutingPolicySecretBindRequest,
    SearchProfileCreateRequest, SearchProfileDefaultDomainRequest,
    SearchProfileDomainAllowlistRequest, SearchProfileIndexerSetRequest,
    SearchProfilePolicySetRequest, SearchProfileResponse, SearchProfileTagSetRequest,
    SearchProfileUpdateRequest, SecretCreateRequest, SecretResponse, TagCreateRequest,
    TagDeleteRequest, TagResponse, TagUpdateRequest, TorznabInstanceCreateRequest,
    TorznabInstanceResponse, TorznabInstanceStateRequest, TrackerCategoryMappingDeleteRequest,
    TrackerCategoryMappingUpsertRequest,
};
use crate::services::api::{ApiClient, ApiError};

pub(crate) async fn fetch_definitions(
    client: &ApiClient,
) -> Result<IndexerDefinitionListResponse, ApiError> {
    client.get_api("/v1/indexers/definitions").await
}

pub(crate) async fn import_cardigann_definition(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<CardigannDefinitionImportResponse, String> {
    let request = CardigannDefinitionImportRequest {
        yaml_payload: draft.cardigann_yaml_payload.clone(),
        is_deprecated: Some(draft.cardigann_is_deprecated),
    };
    client
        .post_api("/v1/indexers/definitions/import/cardigann", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn export_backup_snapshot(
    client: &ApiClient,
) -> Result<IndexerBackupSnapshot, String> {
    let response: IndexerBackupExportResponse = client
        .get_api("/v1/indexers/backup/export")
        .await
        .map_err(|err| err.to_string())?;
    Ok(response.snapshot)
}

pub(crate) async fn restore_backup_snapshot(
    client: &ApiClient,
    snapshot: &IndexerBackupSnapshot,
) -> Result<IndexerBackupRestoreResponse, String> {
    let request = IndexerBackupRestoreRequest {
        snapshot: snapshot.clone(),
    };
    client
        .post_api("/v1/indexers/backup/restore", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_tag(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<TagResponse, String> {
    let request = TagCreateRequest {
        tag_key: draft.tag_key.trim().to_string(),
        display_name: draft.tag_display_name.trim().to_string(),
    };
    client
        .post_api("/v1/indexers/tags", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_health_notification_hooks(
    client: &ApiClient,
) -> Result<IndexerHealthNotificationHookListResponse, String> {
    client
        .get_api("/v1/indexers/health-notifications")
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_health_notification_hook(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerHealthNotificationHookResponse, String> {
    let request = IndexerHealthNotificationHookCreateRequest {
        channel: draft.health_notification_channel.trim().to_string(),
        display_name: draft.health_notification_display_name.trim().to_string(),
        status_threshold: draft
            .health_notification_status_threshold
            .trim()
            .to_string(),
        webhook_url: optional_string(&draft.health_notification_webhook_url),
        email: optional_string(&draft.health_notification_email),
    };
    client
        .post_api("/v1/indexers/health-notifications", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn update_health_notification_hook(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerHealthNotificationHookResponse, String> {
    let request = IndexerHealthNotificationHookUpdateRequest {
        indexer_health_notification_hook_public_id: required_uuid(
            &draft.health_notification_hook_public_id,
        )?,
        display_name: optional_string(&draft.health_notification_display_name),
        status_threshold: optional_string(&draft.health_notification_status_threshold),
        webhook_url: optional_string(&draft.health_notification_webhook_url),
        email: optional_string(&draft.health_notification_email),
        is_enabled: Some(draft.health_notification_is_enabled),
    };
    client
        .patch_api("/v1/indexers/health-notifications", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn delete_health_notification_hook(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let request = IndexerHealthNotificationHookDeleteRequest {
        indexer_health_notification_hook_public_id: required_uuid(
            &draft.health_notification_hook_public_id,
        )?,
    };
    client
        .delete_api_empty("/v1/indexers/health-notifications", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn update_tag(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<TagResponse, String> {
    let request = TagUpdateRequest {
        tag_public_id: optional_uuid(&draft.tag_public_id)?,
        tag_key: optional_string(&draft.tag_key),
        display_name: draft.tag_display_name.trim().to_string(),
    };
    client
        .patch_api("/v1/indexers/tags", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn delete_tag(client: &ApiClient, draft: &IndexersDraft) -> Result<(), String> {
    if let Some(tag_key) = optional_string(&draft.tag_key) {
        let encoded = urlencoding::encode(&tag_key);
        client
            .delete_path(&format!("/v1/indexers/tags/{encoded}"))
            .await
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    let request = TagDeleteRequest {
        tag_public_id: optional_uuid(&draft.tag_public_id)?,
        tag_key: optional_string(&draft.tag_key),
    };
    let _: TagResponse = client
        .delete_api("/v1/indexers/tags", &request)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(crate) async fn create_secret(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SecretResponse, String> {
    let request = SecretCreateRequest {
        secret_type: draft.secret_type.trim().to_string(),
        secret_value: draft.secret_value.clone(),
    };
    client
        .post_api("/v1/indexers/secrets", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn rotate_secret(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SecretResponse, String> {
    let request = crate::models::SecretRotateRequest {
        secret_public_id: required_uuid(&draft.secret_public_id)?,
        secret_value: draft.secret_new_value.clone(),
    };
    client
        .patch_api("/v1/indexers/secrets", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn revoke_secret(client: &ApiClient, draft: &IndexersDraft) -> Result<(), String> {
    let request = crate::models::SecretRevokeRequest {
        secret_public_id: required_uuid(&draft.secret_public_id)?,
    };
    let _: SecretResponse = client
        .delete_api("/v1/indexers/secrets", &request)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(crate) async fn create_routing_policy(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RoutingPolicyResponse, String> {
    let request = RoutingPolicyCreateRequest {
        display_name: draft.routing_display_name.trim().to_string(),
        mode: draft.routing_mode.trim().to_string(),
    };
    client
        .post_api("/v1/indexers/routing-policies", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_routing_policy(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RoutingPolicyDetailResponse, String> {
    let routing_policy_public_id = required_uuid(&draft.routing_policy_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/routing-policies/{routing_policy_public_id}"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_routing_param(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RoutingPolicyResponse, String> {
    let routing_policy_public_id = required_uuid(&draft.routing_policy_public_id)?;
    let request = RoutingPolicyParamSetRequest {
        param_key: draft.routing_param_key.trim().to_string(),
        value_plain: optional_string(&draft.routing_param_plain),
        value_int: optional_i32(&draft.routing_param_int)?,
        value_bool: optional_bool(&draft.routing_param_bool)?,
    };
    client
        .post_api(
            &format!("/v1/indexers/routing-policies/{routing_policy_public_id}/params"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn bind_routing_secret(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RoutingPolicyResponse, String> {
    let routing_policy_public_id = required_uuid(&draft.routing_policy_public_id)?;
    let request = RoutingPolicySecretBindRequest {
        param_key: draft.routing_param_key.trim().to_string(),
        secret_public_id: required_uuid(&draft.routing_secret_public_id)?,
    };
    client
        .post_api(
            &format!("/v1/indexers/routing-policies/{routing_policy_public_id}/secrets"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_rate_limit_policy(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RateLimitPolicyResponse, String> {
    let request = RateLimitPolicyCreateRequest {
        display_name: draft.rate_limit_display_name.trim().to_string(),
        rpm: required_i32(&draft.rate_limit_rpm)?,
        burst: required_i32(&draft.rate_limit_burst)?,
        concurrent: required_i32(&draft.rate_limit_concurrent)?,
    };
    client
        .post_api("/v1/indexers/rate-limits", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn update_rate_limit_policy(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RateLimitPolicyResponse, String> {
    let rate_limit_policy_public_id = required_uuid(&draft.rate_limit_public_id)?;
    let request = RateLimitPolicyUpdateRequest {
        display_name: optional_string(&draft.rate_limit_display_name),
        rpm: optional_i32(&draft.rate_limit_rpm)?,
        burst: optional_i32(&draft.rate_limit_burst)?,
        concurrent: optional_i32(&draft.rate_limit_concurrent)?,
    };
    client
        .patch_api(
            &format!("/v1/indexers/rate-limits/{rate_limit_policy_public_id}"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn delete_rate_limit_policy(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let rate_limit_policy_public_id = required_uuid(&draft.rate_limit_public_id)?;
    let _: RateLimitPolicyResponse = client
        .delete_api(
            &format!("/v1/indexers/rate-limits/{rate_limit_policy_public_id}"),
            &serde_json::json!({}),
        )
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(crate) async fn assign_rate_limit_to_indexer(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.rate_limit_indexer_public_id)?;
    let request = RateLimitPolicyAssignmentRequest {
        rate_limit_policy_public_id: optional_uuid(&draft.rate_limit_public_id)?,
    };
    client
        .put_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/rate-limit"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn assign_rate_limit_to_routing(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<RoutingPolicyResponse, String> {
    let routing_policy_public_id = required_uuid(&draft.rate_limit_routing_public_id)?;
    let request = RateLimitPolicyAssignmentRequest {
        rate_limit_policy_public_id: optional_uuid(&draft.rate_limit_public_id)?,
    };
    client
        .put_api(
            &format!("/v1/indexers/routing-policies/{routing_policy_public_id}/rate-limit"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn upsert_tracker_category_mapping(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let request = TrackerCategoryMappingUpsertRequest {
        torznab_instance_public_id: optional_uuid(
            &draft.category_mapping_torznab_instance_public_id,
        )?,
        indexer_definition_upstream_slug: optional_string(
            &draft.category_mapping_definition_upstream_slug,
        ),
        indexer_instance_public_id: optional_uuid(
            &draft.category_mapping_indexer_instance_public_id,
        )?,
        tracker_category: required_i32(&draft.category_mapping_tracker_category)?,
        tracker_subcategory: optional_i32(&draft.category_mapping_tracker_subcategory)?,
        torznab_cat_id: required_i32(&draft.category_mapping_torznab_cat_id)?,
        media_domain_key: optional_string(&draft.category_mapping_media_domain_key),
    };
    client
        .post_api_empty("/v1/indexers/category-mappings/tracker", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn delete_tracker_category_mapping(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let request = TrackerCategoryMappingDeleteRequest {
        torznab_instance_public_id: optional_uuid(
            &draft.category_mapping_torznab_instance_public_id,
        )?,
        indexer_definition_upstream_slug: optional_string(
            &draft.category_mapping_definition_upstream_slug,
        ),
        indexer_instance_public_id: optional_uuid(
            &draft.category_mapping_indexer_instance_public_id,
        )?,
        tracker_category: required_i32(&draft.category_mapping_tracker_category)?,
        tracker_subcategory: optional_i32(&draft.category_mapping_tracker_subcategory)?,
    };
    client
        .delete_api_empty("/v1/indexers/category-mappings/tracker", &request)
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(crate) async fn create_indexer_instance(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let request = IndexerInstanceCreateRequest {
        indexer_definition_upstream_slug: draft.indexer_definition_upstream_slug.trim().to_string(),
        display_name: draft.indexer_display_name.trim().to_string(),
        priority: optional_i32(&draft.indexer_priority)?,
        trust_tier_key: optional_string(&draft.indexer_trust_tier_key),
        routing_policy_public_id: optional_uuid(&draft.indexer_routing_policy_public_id)?,
    };
    client
        .post_api("/v1/indexers/instances", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn update_indexer_instance(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerInstanceUpdateRequest {
        display_name: optional_string(&draft.indexer_display_name),
        priority: optional_i32(&draft.indexer_priority)?,
        trust_tier_key: optional_string(&draft.indexer_trust_tier_key),
        routing_policy_public_id: optional_uuid(&draft.indexer_routing_policy_public_id)?,
        is_enabled: Some(draft.indexer_is_enabled),
        enable_rss: Some(draft.indexer_enable_rss),
        enable_automatic_search: Some(draft.indexer_enable_automatic_search),
        enable_interactive_search: Some(draft.indexer_enable_interactive_search),
    };
    client
        .patch_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_indexer_media_domains(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerInstanceMediaDomainsRequest {
        media_domain_keys: string_list(&draft.indexer_media_domain_keys),
    };
    client
        .put_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/media-domains"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_indexer_rss_subscription(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerRssSubscriptionResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/instances/{indexer_instance_public_id}/rss"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn update_indexer_rss_subscription(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerRssSubscriptionResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerRssSubscriptionUpdateRequest {
        is_enabled: draft.indexer_enable_rss,
        interval_seconds: optional_i32(&draft.indexer_rss_interval_seconds)?,
    };
    client
        .put_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/rss"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_indexer_rss_items(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerRssSeenItemsResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let limit = optional_i32(&draft.indexer_rss_recent_limit)?;
    let path = match limit {
        Some(limit) => {
            format!("/v1/indexers/instances/{indexer_instance_public_id}/rss/items?limit={limit}")
        }
        None => format!("/v1/indexers/instances/{indexer_instance_public_id}/rss/items"),
    };
    client.get_api(&path).await.map_err(|err| err.to_string())
}

pub(crate) async fn mark_indexer_rss_item_seen(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerRssSeenMarkResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerRssSeenMarkRequest {
        item_guid: optional_string(&draft.indexer_rss_item_guid),
        infohash_v1: optional_string(&draft.indexer_rss_infohash_v1),
        infohash_v2: optional_string(&draft.indexer_rss_infohash_v2),
        magnet_hash: optional_string(&draft.indexer_rss_magnet_hash),
    };
    client
        .post_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/rss/items"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_indexer_connectivity_profile(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerConnectivityProfileResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/instances/{indexer_instance_public_id}/connectivity-profile"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_indexer_source_reputation(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerSourceReputationListResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let limit = optional_i32(&draft.indexer_reputation_limit)?;
    let mut path = format!(
        "/v1/indexers/instances/{indexer_instance_public_id}/reputation?window_key={}",
        draft.indexer_reputation_window.trim()
    );
    if let Some(limit) = limit {
        path.push_str(&format!("&limit={limit}"));
    }
    client.get_api(&path).await.map_err(|err| err.to_string())
}

pub(crate) async fn fetch_indexer_health_events(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerHealthEventListResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let limit = optional_i32(&draft.indexer_health_event_limit)?;
    let mut path = format!("/v1/indexers/instances/{indexer_instance_public_id}/health-events");
    if let Some(limit) = limit {
        path.push_str(&format!("?limit={limit}"));
    }
    client.get_api(&path).await.map_err(|err| err.to_string())
}

pub(crate) async fn set_indexer_tags(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let tag_keys = string_list(&draft.indexer_tag_keys);
    let request = IndexerInstanceTagsRequest {
        tag_public_ids: None,
        tag_keys: (!tag_keys.is_empty()).then_some(tag_keys),
    };
    client
        .put_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/tags"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_indexer_field_value(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerInstanceFieldValueRequest {
        field_name: draft.indexer_field_name.trim().to_string(),
        value_plain: optional_string(&draft.indexer_field_plain),
        value_int: optional_i32(&draft.indexer_field_int)?,
        value_decimal: optional_string(&draft.indexer_field_decimal),
        value_bool: optional_bool(&draft.indexer_field_bool)?,
    };
    client
        .patch_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/fields/value"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn bind_indexer_field_secret(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerInstanceFieldSecretBindRequest {
        field_name: draft.indexer_field_name.trim().to_string(),
        secret_public_id: required_uuid(&draft.indexer_field_secret_public_id)?,
    };
    client
        .patch_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/fields/secret"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_cf_state(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerCfStateResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/instances/{indexer_instance_public_id}/cf-state"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn reset_cf_state(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerCfStateResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerCfStateResetRequest {
        reason: draft.cf_reset_reason.trim().to_string(),
    };
    client
        .post_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/cf-state/reset"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn prepare_indexer_test(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceTestPrepareResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    client
        .post_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/test/prepare"),
            &serde_json::json!({}),
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn finalize_indexer_test(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerInstanceTestFinalizeResponse, String> {
    let indexer_instance_public_id = required_uuid(&draft.indexer_instance_public_id)?;
    let request = IndexerInstanceTestFinalizeRequest {
        ok: draft.test_ok,
        error_class: optional_string(&draft.test_error_class),
        error_code: optional_string(&draft.test_error_code),
        detail: optional_string(&draft.test_detail),
        result_count: optional_i32(&draft.test_result_count)?,
    };
    client
        .post_api(
            &format!("/v1/indexers/instances/{indexer_instance_public_id}/test/finalize"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_search_profile(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SearchProfileResponse, String> {
    let request = SearchProfileCreateRequest {
        display_name: draft.search_profile_display_name.trim().to_string(),
        is_default: Some(draft.search_profile_is_default),
        page_size: optional_i32(&draft.search_profile_page_size)?,
        default_media_domain_key: optional_string(&draft.search_profile_default_media_domain_key),
        user_public_id: None,
    };
    client
        .post_api("/v1/indexers/search-profiles", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn provision_app_sync(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<AppSyncProvisionSummary, String> {
    let mut working = draft.clone();
    let created_search_profile = optional_uuid(&working.search_profile_public_id)?.is_none();
    let search_profile_public_id = if created_search_profile {
        let response = create_search_profile(client, &working).await?;
        working.search_profile_public_id = response.search_profile_public_id.to_string();
        response.search_profile_public_id
    } else {
        required_uuid(&working.search_profile_public_id)?
    };

    let media_domain_keys = string_list(&working.search_profile_media_domain_keys);
    let allowed_indexer_public_ids = if working.search_profile_indexer_public_ids.trim().is_empty()
    {
        Vec::new()
    } else {
        uuid_list(&working.search_profile_indexer_public_ids)?
    };
    let allowed_tag_keys = string_list(&working.search_profile_tag_keys_allow);
    let blocked_tag_keys = string_list(&working.search_profile_tag_keys_block);
    let preferred_tag_keys = string_list(&working.search_profile_tag_keys_prefer);

    if !working
        .search_profile_default_media_domain_key
        .trim()
        .is_empty()
    {
        let _ = set_search_profile_default_domain(client, &working).await?;
    }
    if !media_domain_keys.is_empty() {
        let _ = set_search_profile_media_domains(client, &working).await?;
    }
    if !working
        .search_profile_policy_set_public_id
        .trim()
        .is_empty()
    {
        let _ = add_search_profile_policy_set(client, &working).await?;
    }
    if !allowed_indexer_public_ids.is_empty() {
        let _ = set_search_profile_indexers(client, &working, "allow").await?;
    }
    if !allowed_tag_keys.is_empty() {
        let _ = set_search_profile_tags(
            client,
            &working,
            "allow",
            &working.search_profile_tag_keys_allow,
        )
        .await?;
    }
    if !blocked_tag_keys.is_empty() {
        let _ = set_search_profile_tags(
            client,
            &working,
            "block",
            &working.search_profile_tag_keys_block,
        )
        .await?;
    }
    if !preferred_tag_keys.is_empty() {
        let _ = set_search_profile_tags(
            client,
            &working,
            "prefer",
            &working.search_profile_tag_keys_prefer,
        )
        .await?;
    }

    working.torznab_search_profile_public_id = search_profile_public_id.to_string();
    let torznab = create_torznab_instance(client, &working).await?;
    working.torznab_instance_public_id = torznab.torznab_instance_public_id.to_string();
    if !working.torznab_is_enabled {
        let _ = set_torznab_state(client, &working).await?;
    }

    Ok(AppSyncProvisionSummary {
        search_profile_public_id,
        created_search_profile,
        torznab_instance_public_id: torznab.torznab_instance_public_id,
        torznab_api_key_plaintext: torznab.api_key_plaintext,
        default_media_domain_key: optional_string(&working.search_profile_default_media_domain_key),
        media_domain_keys,
        allowed_indexer_public_ids,
        allowed_tag_keys,
        blocked_tag_keys,
        preferred_tag_keys,
    })
}

pub(crate) async fn update_search_profile(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let request = SearchProfileUpdateRequest {
        display_name: optional_string(&draft.search_profile_display_name),
        page_size: optional_i32(&draft.search_profile_page_size)?,
    };
    client
        .patch_api(
            &format!("/v1/indexers/search-profiles/{search_profile_public_id}"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_search_profile_default_domain(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let request = SearchProfileDefaultDomainRequest {
        default_media_domain_key: optional_string(&draft.search_profile_default_media_domain_key),
    };
    client
        .put_api(
            &format!("/v1/indexers/search-profiles/{search_profile_public_id}/default-domain"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_search_profile_media_domains(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let request = SearchProfileDomainAllowlistRequest {
        media_domain_keys: string_list(&draft.search_profile_media_domain_keys),
    };
    client
        .put_api(
            &format!("/v1/indexers/search-profiles/{search_profile_public_id}/media-domains"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn add_search_profile_policy_set(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let request = SearchProfilePolicySetRequest {
        policy_set_public_id: required_uuid(&draft.search_profile_policy_set_public_id)?,
    };
    client
        .post_api(
            &format!("/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_search_profile_indexers(
    client: &ApiClient,
    draft: &IndexersDraft,
    path_suffix: &str,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let request = SearchProfileIndexerSetRequest {
        indexer_instance_public_ids: uuid_list(&draft.search_profile_indexer_public_ids)?,
    };
    client
        .put_api(
            &format!(
                "/v1/indexers/search-profiles/{search_profile_public_id}/indexers/{path_suffix}"
            ),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_search_profile_tags(
    client: &ApiClient,
    draft: &IndexersDraft,
    path_suffix: &str,
    tag_value: &str,
) -> Result<SearchProfileResponse, String> {
    let search_profile_public_id = required_uuid(&draft.search_profile_public_id)?;
    let tag_keys = string_list(tag_value);
    let request = SearchProfileTagSetRequest {
        tag_public_ids: None,
        tag_keys: (!tag_keys.is_empty()).then_some(tag_keys),
    };
    client
        .put_api(
            &format!("/v1/indexers/search-profiles/{search_profile_public_id}/tags/{path_suffix}"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_policy_set(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<PolicySetResponse, String> {
    let request = PolicySetCreateRequest {
        display_name: draft.policy_set_display_name.trim().to_string(),
        scope: draft.policy_set_scope.trim().to_string(),
        enabled: None,
    };
    client
        .post_api("/v1/indexers/policies", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_policy_rule(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<PolicyRuleResponse, String> {
    let policy_set_public_id = required_uuid(&draft.policy_set_public_id)?;
    let value_set_items = {
        let items = string_list(&draft.policy_value_set_text);
        (!items.is_empty()).then_some(
            items
                .into_iter()
                .map(|value_text| PolicyRuleValueItemRequest {
                    value_text: Some(value_text),
                    value_int: None,
                    value_bigint: None,
                    value_uuid: None,
                })
                .collect::<Vec<PolicyRuleValueItemRequest>>(),
        )
    };
    let request = PolicyRuleCreateRequest {
        rule_type: draft.policy_rule_type.trim().to_string(),
        match_field: draft.policy_match_field.trim().to_string(),
        match_operator: draft.policy_match_operator.trim().to_string(),
        sort_order: required_i32(&draft.policy_sort_order)?,
        match_value_text: optional_string(&draft.policy_match_value_text),
        match_value_int: optional_i32(&draft.policy_match_value_int)?,
        match_value_uuid: optional_uuid(&draft.policy_match_value_uuid)?,
        value_set_items,
        action: draft.policy_action.trim().to_string(),
        severity: draft.policy_severity.trim().to_string(),
        is_case_insensitive: Some(draft.policy_is_case_insensitive),
        rationale: optional_string(&draft.policy_rationale),
        expires_at: None,
    };
    client
        .post_api(
            &format!("/v1/indexers/policies/{policy_set_public_id}/rules"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_import_job(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<ImportJobResponse, String> {
    let request = ImportJobCreateRequest {
        source: draft.import_job_source.trim().to_string(),
        is_dry_run: Some(draft.import_dry_run),
        target_search_profile_public_id: optional_uuid(&draft.torznab_search_profile_public_id)?,
        target_torznab_instance_public_id: optional_uuid(&draft.torznab_instance_public_id)?,
    };
    client
        .post_api("/v1/indexers/import-jobs", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn run_import_job_api(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<ImportJobResponse, String> {
    let import_job_public_id = required_uuid(&draft.import_job_public_id)?;
    let request = ImportJobRunProwlarrApiRequest {
        prowlarr_url: draft.prowlarr_base_url.trim().to_string(),
        prowlarr_api_key_secret_public_id: required_uuid(&draft.prowlarr_api_key)?,
    };
    client
        .post_api(
            &format!("/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn run_import_job_backup(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<ImportJobResponse, String> {
    let import_job_public_id = required_uuid(&draft.import_job_public_id)?;
    let request = ImportJobRunProwlarrBackupRequest {
        backup_blob_ref: draft.prowlarr_backup_payload.trim().to_string(),
    };
    client
        .post_api(
            &format!("/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_import_job_status(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<ImportJobStatusResponse, String> {
    let import_job_public_id = required_uuid(&draft.import_job_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/import-jobs/{import_job_public_id}/status"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_import_job_results(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<ImportJobResultsResponse, String> {
    let import_job_public_id = required_uuid(&draft.import_job_public_id)?;
    client
        .get_api(&format!(
            "/v1/indexers/import-jobs/{import_job_public_id}/results"
        ))
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn fetch_source_metadata_conflicts(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<IndexerSourceMetadataConflictListResponse, String> {
    let limit = optional_i32(&draft.source_conflict_limit)?;
    let mut path = format!(
        "/v1/indexers/conflicts?include_resolved={}",
        draft.source_conflict_include_resolved
    );
    if let Some(limit) = limit {
        path.push_str(&format!("&limit={limit}"));
    }
    client.get_api(&path).await.map_err(|err| err.to_string())
}

pub(crate) async fn resolve_source_metadata_conflict(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let request = IndexerSourceMetadataConflictResolveRequest {
        conflict_id: required_i64(&draft.source_conflict_id)?,
        resolution: draft.source_conflict_resolution.trim().to_string(),
        resolution_note: optional_string(&draft.source_conflict_resolution_note),
    };
    client
        .patch_api::<_, ()>("/v1/indexers/conflicts", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn reopen_source_metadata_conflict(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let request = IndexerSourceMetadataConflictReopenRequest {
        conflict_id: required_i64(&draft.source_conflict_id)?,
        resolution_note: optional_string(&draft.source_conflict_resolution_note),
    };
    client
        .post_api_empty("/v1/indexers/conflicts/reopen", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn create_torznab_instance(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<TorznabInstanceResponse, String> {
    let request = TorznabInstanceCreateRequest {
        search_profile_public_id: required_uuid(&draft.torznab_search_profile_public_id)?,
        display_name: draft.torznab_display_name.trim().to_string(),
    };
    client
        .post_api("/v1/indexers/torznab-instances", &request)
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn rotate_torznab_key(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<TorznabInstanceResponse, String> {
    let torznab_instance_public_id = required_uuid(&draft.torznab_instance_public_id)?;
    client
        .patch_api(
            &format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}/rotate"),
            &serde_json::json!({}),
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn set_torznab_state(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<TorznabInstanceResponse, String> {
    let torznab_instance_public_id = required_uuid(&draft.torznab_instance_public_id)?;
    let request = TorznabInstanceStateRequest {
        is_enabled: draft.torznab_is_enabled,
    };
    client
        .put_api(
            &format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}/state"),
            &request,
        )
        .await
        .map_err(|err| err.to_string())
}

pub(crate) async fn delete_torznab_instance(
    client: &ApiClient,
    draft: &IndexersDraft,
) -> Result<(), String> {
    let torznab_instance_public_id = required_uuid(&draft.torznab_instance_public_id)?;
    let _: TorznabInstanceResponse = client
        .delete_api(
            &format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}"),
            &serde_json::json!({}),
        )
        .await
        .map_err(|err| err.to_string())?;
    Ok(())
}
