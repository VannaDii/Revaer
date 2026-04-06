use std::fmt::Write as _;
use std::fs;

use anyhow::anyhow;
use reqwest::Method;
use revaer_api::models::{
    ImportJobCreateRequest, ImportJobResponse, ImportJobResultsResponse,
    ImportJobRunProwlarrApiRequest, ImportJobRunProwlarrBackupRequest, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerBackupRestoreRequest, IndexerBackupRestoreResponse,
    IndexerConnectivityProfileResponse, IndexerHealthEventListResponse,
    IndexerHealthNotificationHookCreateRequest, IndexerHealthNotificationHookDeleteRequest,
    IndexerHealthNotificationHookListResponse, IndexerHealthNotificationHookResponse,
    IndexerHealthNotificationHookUpdateRequest, IndexerInstanceListResponse,
    IndexerInstanceTestFinalizeRequest, IndexerInstanceTestFinalizeResponse,
    IndexerInstanceTestPrepareResponse, IndexerRssSeenItemsResponse, IndexerRssSeenMarkRequest,
    IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerRssSubscriptionUpdateRequest, IndexerSourceReputationListResponse,
    MediaDomainMappingDeleteRequest, MediaDomainMappingUpsertRequest, PolicyRuleCreateRequest,
    PolicyRuleReorderRequest, PolicyRuleResponse, PolicyRuleValueItemRequest,
    PolicySetCreateRequest, PolicySetListResponse, PolicySetReorderRequest, PolicySetResponse,
    PolicySetUpdateRequest, RateLimitPolicyAssignmentRequest, RateLimitPolicyCreateRequest,
    RateLimitPolicyListResponse, RateLimitPolicyResponse, RateLimitPolicyUpdateRequest,
    RoutingPolicyCreateRequest, RoutingPolicyDetailResponse, RoutingPolicyListResponse,
    RoutingPolicyParamSetRequest, RoutingPolicyResponse, RoutingPolicySecretBindRequest,
    SearchProfileCreateRequest, SearchProfileDefaultDomainRequest, SearchProfileDefaultRequest,
    SearchProfileDomainAllowlistRequest, SearchProfileIndexerSetRequest, SearchProfileListResponse,
    SearchProfilePolicySetRequest, SearchProfileResponse, SearchProfileTagSetRequest,
    SearchProfileUpdateRequest, SecretCreateRequest, SecretMetadataListResponse, SecretResponse,
    SecretRevokeRequest, SecretRotateRequest, TagCreateRequest, TagDeleteRequest, TagListResponse,
    TagResponse, TagUpdateRequest, TorznabInstanceCreateRequest, TorznabInstanceListResponse,
    TorznabInstanceResponse, TorznabInstanceStateRequest, TrackerCategoryMappingDeleteRequest,
    TrackerCategoryMappingUpsertRequest,
};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::cli::{
    BackupRestoreArgs, HealthNotificationCreateArgs, HealthNotificationDeleteArgs,
    HealthNotificationUpdateArgs, ImportJobCreateArgs, ImportJobResultsArgs,
    ImportJobRunProwlarrApiArgs, ImportJobRunProwlarrBackupArgs, ImportJobStatusArgs,
    IndexerInstanceReadArgs, IndexerInstanceRssItemsArgs, IndexerInstanceTestFinalizeArgs,
    IndexerInstanceTestPrepareArgs, IndexerRoutingPolicyReadArgs, IndexerRssMarkSeenArgs,
    IndexerRssSetArgs, MediaDomainMappingDeleteArgs, MediaDomainMappingUpsertArgs, OutputFormat,
    PolicyRuleCreateArgs, PolicyRuleDisableArgs, PolicyRuleEnableArgs, PolicyRuleReorderArgs,
    PolicySetCreateArgs, PolicySetDisableArgs, PolicySetEnableArgs, PolicySetReorderArgs,
    PolicySetUpdateArgs, RateLimitAssignInstanceArgs, RateLimitAssignRoutingArgs,
    RateLimitCreateArgs, RateLimitDeleteArgs, RateLimitUpdateArgs, RoutingPolicyBindSecretArgs,
    RoutingPolicyCreateArgs, RoutingPolicySetParamArgs, SearchProfileCreateArgs,
    SearchProfileIndexerSetArgs, SearchProfilePolicySetArgs, SearchProfileSetDefaultArgs,
    SearchProfileSetDefaultDomainArgs, SearchProfileSetMediaDomainsArgs, SearchProfileTagSetArgs,
    SearchProfileUpdateArgs, SecretCreateArgs, SecretRevokeArgs, SecretRotateArgs, TagCreateArgs,
    TagDeleteArgs, TagUpdateArgs, TorznabCreateArgs, TorznabDeleteArgs, TorznabRotateArgs,
    TorznabSetStateArgs, TrackerCategoryMappingDeleteArgs, TrackerCategoryMappingUpsertArgs,
};
use crate::client::{AppContext, CliError, CliResult, HEADER_API_KEY, classify_problem};
use crate::output::{
    render_import_job_results, render_import_job_status, render_import_job_summary,
    render_indexer_backup_export, render_indexer_backup_restore,
    render_indexer_connectivity_profile, render_indexer_health_events,
    render_indexer_health_notification_hook_list, render_indexer_health_notification_hook_response,
    render_indexer_instance_list, render_indexer_instance_test_finalize,
    render_indexer_instance_test_prepare, render_indexer_rss_seen_items,
    render_indexer_rss_seen_mark, render_indexer_rss_subscription,
    render_indexer_source_reputation_list, render_policy_rule_response, render_policy_set_list,
    render_policy_set_response, render_rate_limit_policy_list, render_rate_limit_policy_response,
    render_routing_policy_detail, render_routing_policy_list, render_routing_policy_response,
    render_search_profile_list, render_search_profile_response, render_secret_metadata_list,
    render_secret_response, render_tag_list, render_tag_response, render_torznab_instance,
    render_torznab_instance_list,
};

pub(crate) async fn handle_tag_create(
    ctx: &AppContext,
    args: TagCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let tag_key = args.tag_key.trim();
    if tag_key.is_empty() {
        return Err(CliError::validation("tag key must not be empty"));
    }
    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }

    let request = TagCreateRequest {
        tag_key: tag_key.to_string(),
        display_name: display_name.to_string(),
    };
    let response: TagResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/tags",
        "/v1/indexers/tags",
        &request,
    )
    .await?;
    render_tag_response(&response, output)
}

pub(crate) async fn handle_tag_update(
    ctx: &AppContext,
    args: TagUpdateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }

    let request = TagUpdateRequest {
        tag_public_id: args.tag_public_id,
        tag_key: trim_optional_text(args.tag_key.as_deref()),
        display_name: display_name.to_string(),
    };
    let response: TagResponse = send_indexer_json(
        ctx,
        Method::PATCH,
        "/v1/indexers/tags",
        "/v1/indexers/tags",
        &request,
    )
    .await?;
    render_tag_response(&response, output)
}

pub(crate) async fn handle_tag_delete(ctx: &AppContext, args: TagDeleteArgs) -> CliResult<()> {
    let request = TagDeleteRequest {
        tag_public_id: args.tag_public_id,
        tag_key: trim_optional_text(args.tag_key.as_deref()),
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        "/v1/indexers/tags",
        "/v1/indexers/tags",
        &request,
    )
    .await?;
    println!("Tag deleted");
    Ok(())
}

pub(crate) async fn handle_secret_create(
    ctx: &AppContext,
    args: SecretCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let secret_type = args.secret_type.trim();
    if secret_type.is_empty() {
        return Err(CliError::validation("secret type must not be empty"));
    }

    let request = SecretCreateRequest {
        secret_type: secret_type.to_string(),
        secret_value: args.secret_value,
    };
    let response: SecretResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/secrets",
        "/v1/indexers/secrets",
        &request,
    )
    .await?;
    render_secret_response(&response, output)
}

pub(crate) async fn handle_secret_rotate(
    ctx: &AppContext,
    args: SecretRotateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = SecretRotateRequest {
        secret_public_id: args.secret_public_id,
        secret_value: args.secret_value,
    };
    let response: SecretResponse = send_indexer_json(
        ctx,
        Method::PATCH,
        "/v1/indexers/secrets",
        "/v1/indexers/secrets",
        &request,
    )
    .await?;
    render_secret_response(&response, output)
}

pub(crate) async fn handle_secret_revoke(
    ctx: &AppContext,
    args: SecretRevokeArgs,
) -> CliResult<()> {
    let request = SecretRevokeRequest {
        secret_public_id: args.secret_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        "/v1/indexers/secrets",
        "/v1/indexers/secrets",
        &request,
    )
    .await?;
    println!("Secret revoked");
    Ok(())
}

pub(crate) async fn handle_health_notification_hook_create(
    ctx: &AppContext,
    args: HealthNotificationCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let channel = require_trimmed(&args.channel, "channel must not be empty")?;
    let display_name = require_trimmed(&args.display_name, "display name must not be empty")?;
    let status_threshold =
        require_trimmed(&args.status_threshold, "status threshold must not be empty")?;

    let request = IndexerHealthNotificationHookCreateRequest {
        channel,
        display_name,
        status_threshold,
        webhook_url: trim_optional_text(args.webhook_url.as_deref()),
        email: trim_optional_text(args.email.as_deref()),
    };
    let response: IndexerHealthNotificationHookResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/health-notifications",
        "/v1/indexers/health-notifications",
        &request,
    )
    .await?;
    render_indexer_health_notification_hook_response(&response, output)
}

pub(crate) async fn handle_health_notification_hook_update(
    ctx: &AppContext,
    args: HealthNotificationUpdateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = IndexerHealthNotificationHookUpdateRequest {
        indexer_health_notification_hook_public_id: args.indexer_health_notification_hook_public_id,
        display_name: trim_optional_text(args.display_name.as_deref()),
        status_threshold: trim_optional_text(args.status_threshold.as_deref()),
        webhook_url: trim_optional_text(args.webhook_url.as_deref()),
        email: trim_optional_text(args.email.as_deref()),
        is_enabled: args.is_enabled,
    };
    let response: IndexerHealthNotificationHookResponse = send_indexer_json(
        ctx,
        Method::PATCH,
        "/v1/indexers/health-notifications",
        "/v1/indexers/health-notifications",
        &request,
    )
    .await?;
    render_indexer_health_notification_hook_response(&response, output)
}

pub(crate) async fn handle_health_notification_hook_delete(
    ctx: &AppContext,
    args: HealthNotificationDeleteArgs,
) -> CliResult<()> {
    let request = IndexerHealthNotificationHookDeleteRequest {
        indexer_health_notification_hook_public_id: args.indexer_health_notification_hook_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        "/v1/indexers/health-notifications",
        "/v1/indexers/health-notifications",
        &request,
    )
    .await?;
    println!("Health notification hook deleted");
    Ok(())
}

pub(crate) async fn handle_tracker_category_mapping_upsert(
    ctx: &AppContext,
    args: TrackerCategoryMappingUpsertArgs,
) -> CliResult<()> {
    let request = TrackerCategoryMappingUpsertRequest {
        torznab_instance_public_id: args.torznab_instance_public_id,
        indexer_definition_upstream_slug: trim_optional_text(
            args.indexer_definition_upstream_slug.as_deref(),
        ),
        indexer_instance_public_id: args.indexer_instance_public_id,
        tracker_category: args.tracker_category,
        tracker_subcategory: args.tracker_subcategory,
        torznab_cat_id: args.torznab_cat_id,
        media_domain_key: trim_optional_text(args.media_domain_key.as_deref()),
    };
    send_indexer_no_content(
        ctx,
        Method::POST,
        "/v1/indexers/category-mappings/tracker",
        "/v1/indexers/category-mappings/tracker",
        &request,
    )
    .await?;
    println!("Tracker category mapping updated");
    Ok(())
}

pub(crate) async fn handle_tracker_category_mapping_delete(
    ctx: &AppContext,
    args: TrackerCategoryMappingDeleteArgs,
) -> CliResult<()> {
    let request = TrackerCategoryMappingDeleteRequest {
        torznab_instance_public_id: args.torznab_instance_public_id,
        indexer_definition_upstream_slug: trim_optional_text(
            args.indexer_definition_upstream_slug.as_deref(),
        ),
        indexer_instance_public_id: args.indexer_instance_public_id,
        tracker_category: args.tracker_category,
        tracker_subcategory: args.tracker_subcategory,
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        "/v1/indexers/category-mappings/tracker",
        "/v1/indexers/category-mappings/tracker",
        &request,
    )
    .await?;
    println!("Tracker category mapping deleted");
    Ok(())
}

pub(crate) async fn handle_media_domain_mapping_upsert(
    ctx: &AppContext,
    args: MediaDomainMappingUpsertArgs,
) -> CliResult<()> {
    let media_domain_key = args.media_domain_key.trim();
    if media_domain_key.is_empty() {
        return Err(CliError::validation("media domain key must not be empty"));
    }

    let request = MediaDomainMappingUpsertRequest {
        media_domain_key: media_domain_key.to_string(),
        torznab_cat_id: args.torznab_cat_id,
        is_primary: args.is_primary,
    };
    send_indexer_no_content(
        ctx,
        Method::POST,
        "/v1/indexers/category-mappings/media-domains",
        "/v1/indexers/category-mappings/media-domains",
        &request,
    )
    .await?;
    println!("Media-domain mapping updated");
    Ok(())
}

pub(crate) async fn handle_media_domain_mapping_delete(
    ctx: &AppContext,
    args: MediaDomainMappingDeleteArgs,
) -> CliResult<()> {
    let media_domain_key = args.media_domain_key.trim();
    if media_domain_key.is_empty() {
        return Err(CliError::validation("media domain key must not be empty"));
    }

    let request = MediaDomainMappingDeleteRequest {
        media_domain_key: media_domain_key.to_string(),
        torznab_cat_id: args.torznab_cat_id,
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        "/v1/indexers/category-mappings/media-domains",
        "/v1/indexers/category-mappings/media-domains",
        &request,
    )
    .await?;
    println!("Media-domain mapping deleted");
    Ok(())
}

pub(crate) async fn handle_routing_policy_create(
    ctx: &AppContext,
    args: RoutingPolicyCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }
    let mode = args.mode.trim();
    if mode.is_empty() {
        return Err(CliError::validation("mode must not be empty"));
    }

    let request = RoutingPolicyCreateRequest {
        display_name: display_name.to_string(),
        mode: mode.to_string(),
    };
    let response: RoutingPolicyResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/routing-policies",
        "/v1/indexers/routing-policies",
        &request,
    )
    .await?;
    render_routing_policy_response(&response, output)
}

pub(crate) async fn handle_routing_policy_set_param(
    ctx: &AppContext,
    args: RoutingPolicySetParamArgs,
) -> CliResult<()> {
    let param_key = args.param_key.trim();
    if param_key.is_empty() {
        return Err(CliError::validation("parameter key must not be empty"));
    }

    let request = RoutingPolicyParamSetRequest {
        param_key: param_key.to_string(),
        value_plain: trim_optional_text(args.value_plain.as_deref()),
        value_int: args.value_int,
        value_bool: args.value_bool,
    };
    send_indexer_no_content(
        ctx,
        Method::POST,
        &format!(
            "/v1/indexers/routing-policies/{}/params",
            args.routing_policy_public_id
        ),
        "/v1/indexers/routing-policies/{id}/params",
        &request,
    )
    .await?;
    println!("Routing policy parameter updated");
    Ok(())
}

pub(crate) async fn handle_routing_policy_bind_secret(
    ctx: &AppContext,
    args: RoutingPolicyBindSecretArgs,
) -> CliResult<()> {
    let param_key = args.param_key.trim();
    if param_key.is_empty() {
        return Err(CliError::validation("parameter key must not be empty"));
    }

    let request = RoutingPolicySecretBindRequest {
        param_key: param_key.to_string(),
        secret_public_id: args.secret_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::POST,
        &format!(
            "/v1/indexers/routing-policies/{}/secrets",
            args.routing_policy_public_id
        ),
        "/v1/indexers/routing-policies/{id}/secrets",
        &request,
    )
    .await?;
    println!("Routing policy secret bound");
    Ok(())
}

pub(crate) async fn handle_rate_limit_policy_create(
    ctx: &AppContext,
    args: RateLimitCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }

    let request = RateLimitPolicyCreateRequest {
        display_name: display_name.to_string(),
        rpm: args.rpm,
        burst: args.burst,
        concurrent: args.concurrent,
    };
    let response: RateLimitPolicyResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/rate-limits",
        "/v1/indexers/rate-limits",
        &request,
    )
    .await?;
    render_rate_limit_policy_response(&response, output)
}

pub(crate) async fn handle_rate_limit_policy_update(
    ctx: &AppContext,
    args: RateLimitUpdateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = RateLimitPolicyUpdateRequest {
        display_name: trim_optional_text(args.display_name.as_deref()),
        rpm: args.rpm,
        burst: args.burst,
        concurrent: args.concurrent,
    };
    let response: RateLimitPolicyResponse = send_indexer_json(
        ctx,
        Method::PATCH,
        &format!(
            "/v1/indexers/rate-limits/{}",
            args.rate_limit_policy_public_id
        ),
        "/v1/indexers/rate-limits/{id}",
        &request,
    )
    .await?;
    render_rate_limit_policy_response(&response, output)
}

pub(crate) async fn handle_rate_limit_policy_delete(
    ctx: &AppContext,
    args: RateLimitDeleteArgs,
) -> CliResult<()> {
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        &format!(
            "/v1/indexers/rate-limits/{}",
            args.rate_limit_policy_public_id
        ),
        "/v1/indexers/rate-limits/{id}",
        &(),
    )
    .await?;
    println!("Rate-limit policy deleted");
    Ok(())
}

pub(crate) async fn handle_rate_limit_assign_instance(
    ctx: &AppContext,
    args: RateLimitAssignInstanceArgs,
) -> CliResult<()> {
    let request = RateLimitPolicyAssignmentRequest {
        rate_limit_policy_public_id: args.rate_limit_policy_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::PUT,
        &format!(
            "/v1/indexers/instances/{}/rate-limit",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/rate-limit",
        &request,
    )
    .await?;
    println!("Indexer-instance rate limit updated");
    Ok(())
}

pub(crate) async fn handle_rate_limit_assign_routing(
    ctx: &AppContext,
    args: RateLimitAssignRoutingArgs,
) -> CliResult<()> {
    let request = RateLimitPolicyAssignmentRequest {
        rate_limit_policy_public_id: args.rate_limit_policy_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::PUT,
        &format!(
            "/v1/indexers/routing-policies/{}/rate-limit",
            args.routing_policy_public_id
        ),
        "/v1/indexers/routing-policies/{id}/rate-limit",
        &request,
    )
    .await?;
    println!("Routing-policy rate limit updated");
    Ok(())
}

pub(crate) async fn handle_search_profile_create(
    ctx: &AppContext,
    args: SearchProfileCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }

    let request = SearchProfileCreateRequest {
        display_name: display_name.to_string(),
        is_default: args.is_default.then_some(true),
        page_size: args.page_size,
        default_media_domain_key: trim_optional_text(args.default_media_domain_key.as_deref()),
        user_public_id: args.user_public_id,
    };
    let response: SearchProfileResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/search-profiles",
        "/v1/indexers/search-profiles",
        &request,
    )
    .await?;
    render_search_profile_response(&response, output)
}

pub(crate) async fn handle_search_profile_update(
    ctx: &AppContext,
    args: SearchProfileUpdateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = SearchProfileUpdateRequest {
        display_name: trim_optional_text(args.display_name.as_deref()),
        page_size: args.page_size,
    };
    let response: SearchProfileResponse = send_indexer_json(
        ctx,
        Method::PATCH,
        &format!(
            "/v1/indexers/search-profiles/{}",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}",
        &request,
    )
    .await?;
    render_search_profile_response(&response, output)
}

pub(crate) async fn handle_search_profile_set_default(
    ctx: &AppContext,
    args: SearchProfileSetDefaultArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = SearchProfileDefaultRequest {
        page_size: args.page_size,
    };
    let response: SearchProfileResponse = send_indexer_json(
        ctx,
        Method::POST,
        &format!(
            "/v1/indexers/search-profiles/{}/default",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}/default",
        &request,
    )
    .await?;
    render_search_profile_response(&response, output)
}

pub(crate) async fn handle_search_profile_set_default_domain(
    ctx: &AppContext,
    args: SearchProfileSetDefaultDomainArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = SearchProfileDefaultDomainRequest {
        default_media_domain_key: trim_optional_text(args.default_media_domain_key.as_deref()),
    };
    let response: SearchProfileResponse = send_indexer_json(
        ctx,
        Method::PUT,
        &format!(
            "/v1/indexers/search-profiles/{}/default-domain",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}/default-domain",
        &request,
    )
    .await?;
    render_search_profile_response(&response, output)
}

pub(crate) async fn handle_search_profile_set_media_domains(
    ctx: &AppContext,
    args: SearchProfileSetMediaDomainsArgs,
) -> CliResult<()> {
    let request = SearchProfileDomainAllowlistRequest {
        media_domain_keys: trim_string_values(args.media_domain_keys),
    };
    send_indexer_no_content(
        ctx,
        Method::PUT,
        &format!(
            "/v1/indexers/search-profiles/{}/media-domains",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}/media-domains",
        &request,
    )
    .await?;
    println!("Search-profile media domains updated");
    Ok(())
}

pub(crate) async fn handle_search_profile_add_policy_set(
    ctx: &AppContext,
    args: SearchProfilePolicySetArgs,
) -> CliResult<()> {
    let request = SearchProfilePolicySetRequest {
        policy_set_public_id: args.policy_set_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::POST,
        &format!(
            "/v1/indexers/search-profiles/{}/policy-sets",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}/policy-sets",
        &request,
    )
    .await?;
    println!("Policy set added to search profile");
    Ok(())
}

pub(crate) async fn handle_search_profile_remove_policy_set(
    ctx: &AppContext,
    args: SearchProfilePolicySetArgs,
) -> CliResult<()> {
    let request = SearchProfilePolicySetRequest {
        policy_set_public_id: args.policy_set_public_id,
    };
    send_indexer_no_content(
        ctx,
        Method::DELETE,
        &format!(
            "/v1/indexers/search-profiles/{}/policy-sets",
            args.search_profile_public_id
        ),
        "/v1/indexers/search-profiles/{id}/policy-sets",
        &request,
    )
    .await?;
    println!("Policy set removed from search profile");
    Ok(())
}

pub(crate) async fn handle_search_profile_set_indexer_allow(
    ctx: &AppContext,
    args: SearchProfileIndexerSetArgs,
) -> CliResult<()> {
    handle_search_profile_indexers(
        ctx,
        args.search_profile_public_id,
        "/indexers/allow",
        "search-profile indexer allowlist updated",
        args.indexer_instance_public_ids,
    )
    .await
}

pub(crate) async fn handle_search_profile_set_indexer_block(
    ctx: &AppContext,
    args: SearchProfileIndexerSetArgs,
) -> CliResult<()> {
    handle_search_profile_indexers(
        ctx,
        args.search_profile_public_id,
        "/indexers/block",
        "search-profile indexer blocklist updated",
        args.indexer_instance_public_ids,
    )
    .await
}

pub(crate) async fn handle_search_profile_set_tag_allow(
    ctx: &AppContext,
    args: SearchProfileTagSetArgs,
) -> CliResult<()> {
    handle_search_profile_tags(
        ctx,
        args.search_profile_public_id,
        "/tags/allow",
        "search-profile allowed tags updated",
        args,
    )
    .await
}

pub(crate) async fn handle_search_profile_set_tag_block(
    ctx: &AppContext,
    args: SearchProfileTagSetArgs,
) -> CliResult<()> {
    handle_search_profile_tags(
        ctx,
        args.search_profile_public_id,
        "/tags/block",
        "search-profile blocked tags updated",
        args,
    )
    .await
}

pub(crate) async fn handle_search_profile_set_tag_prefer(
    ctx: &AppContext,
    args: SearchProfileTagSetArgs,
) -> CliResult<()> {
    handle_search_profile_tags(
        ctx,
        args.search_profile_public_id,
        "/tags/prefer",
        "search-profile preferred tags updated",
        args,
    )
    .await
}

pub(crate) async fn handle_indexer_backup_restore(
    ctx: &AppContext,
    args: BackupRestoreArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let snapshot = load_backup_snapshot(&args)?;
    let request = IndexerBackupRestoreRequest { snapshot };
    let response: IndexerBackupRestoreResponse = send_indexer_json(
        ctx,
        Method::POST,
        "/v1/indexers/backup/restore",
        "/v1/indexers/backup/restore",
        &request,
    )
    .await?;
    render_indexer_backup_restore(&response, output)
}

pub(crate) async fn handle_indexer_rss_set(
    ctx: &AppContext,
    args: IndexerRssSetArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = IndexerRssSubscriptionUpdateRequest {
        is_enabled: args.is_enabled,
        interval_seconds: args.interval_seconds,
    };
    let response: IndexerRssSubscriptionResponse = send_indexer_json(
        ctx,
        Method::PUT,
        &format!(
            "/v1/indexers/instances/{}/rss",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/rss",
        &request,
    )
    .await?;
    render_indexer_rss_subscription(&response, output)
}

pub(crate) async fn handle_indexer_rss_mark_seen(
    ctx: &AppContext,
    args: IndexerRssMarkSeenArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let request = IndexerRssSeenMarkRequest {
        item_guid: trim_optional_text(args.item_guid.as_deref()),
        infohash_v1: trim_optional_text(args.infohash_v1.as_deref()),
        infohash_v2: trim_optional_text(args.infohash_v2.as_deref()),
        magnet_hash: trim_optional_text(args.magnet_hash.as_deref()),
    };
    if request.item_guid.is_none()
        && request.infohash_v1.is_none()
        && request.infohash_v2.is_none()
        && request.magnet_hash.is_none()
    {
        return Err(CliError::validation(
            "at least one RSS item identifier must be provided",
        ));
    }

    let response: IndexerRssSeenMarkResponse = send_indexer_json(
        ctx,
        Method::POST,
        &format!(
            "/v1/indexers/instances/{}/rss/items",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/rss/items",
        &request,
    )
    .await?;
    render_indexer_rss_seen_mark(&response, output)
}

pub(crate) async fn handle_import_job_create(
    ctx: &AppContext,
    args: ImportJobCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let source = args.source.as_str();
    if source.trim().is_empty() {
        return Err(CliError::validation("source must not be empty"));
    }

    let request = ImportJobCreateRequest {
        source: source.to_string(),
        is_dry_run: args.dry_run.then_some(true),
        target_search_profile_public_id: args.target_search_profile,
        target_torznab_instance_public_id: args.target_torznab_instance,
    };

    let url = ctx
        .base_url
        .join("/v1/indexers/import-jobs")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /v1/indexers/import-jobs failed: {err}"))
        })?;

    if response.status().is_success() {
        let payload: ImportJobResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse import job response: {err}"))
        })?;
        render_import_job_summary(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_import_job_run_prowlarr_api(
    ctx: &AppContext,
    args: ImportJobRunProwlarrApiArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = ImportJobRunProwlarrApiRequest {
        prowlarr_url: args.prowlarr_url.trim().to_string(),
        prowlarr_api_key_secret_public_id: args.prowlarr_api_key_secret_public_id,
    };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/import-jobs/{}/run/prowlarr-api",
            args.import_job_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/import-jobs/run failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Import job started (id: {})", args.import_job_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_import_job_run_prowlarr_backup(
    ctx: &AppContext,
    args: ImportJobRunProwlarrBackupArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = ImportJobRunProwlarrBackupRequest {
        backup_blob_ref: args.backup_blob_ref.trim().to_string(),
    };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/import-jobs/{}/run/prowlarr-backup",
            args.import_job_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/import-jobs/run failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Import job started (id: {})", args.import_job_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_import_job_status(
    ctx: &AppContext,
    args: ImportJobStatusArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/import-jobs/{}/status",
            args.import_job_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .get(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/import-jobs/status failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let status: ImportJobStatusResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse import job status: {err}"))
        })?;
        render_import_job_status(&status, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_import_job_results(
    ctx: &AppContext,
    args: ImportJobResultsArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/import-jobs/{}/results",
            args.import_job_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .get(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/import-jobs/results failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let results: ImportJobResultsResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse import job results: {err}"))
        })?;
        render_import_job_results(&results, output)
    } else {
        Err(classify_problem(response).await)
    }
}

async fn get_indexer_resource<T>(
    ctx: &AppContext,
    path: &str,
    request_name: &'static str,
) -> CliResult<T>
where
    T: DeserializeOwned,
{
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(path)
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .get(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to {request_name} failed: {err}")))?;

    if response.status().is_success() {
        response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse {request_name} response: {err}"))
        })
    } else {
        Err(classify_problem(response).await)
    }
}

async fn send_indexer_json<TRequest, TResponse>(
    ctx: &AppContext,
    method: Method,
    path: &str,
    request_name: &'static str,
    body: &TRequest,
) -> CliResult<TResponse>
where
    TRequest: Serialize + Sync + ?Sized,
    TResponse: DeserializeOwned,
{
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(path)
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .request(method, url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(body)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to {request_name} failed: {err}")))?;

    if response.status().is_success() {
        response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse {request_name} response: {err}"))
        })
    } else {
        Err(classify_problem(response).await)
    }
}

async fn send_indexer_no_content<TRequest>(
    ctx: &AppContext,
    method: Method,
    path: &str,
    request_name: &'static str,
    body: &TRequest,
) -> CliResult<()>
where
    TRequest: Serialize + Sync + ?Sized,
{
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(path)
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .request(method, url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(body)
        .send()
        .await
        .map_err(|err| CliError::failure(anyhow!("request to {request_name} failed: {err}")))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

async fn handle_search_profile_indexers(
    ctx: &AppContext,
    search_profile_public_id: Uuid,
    suffix: &str,
    success_message: &str,
    indexer_instance_public_ids: Vec<Uuid>,
) -> CliResult<()> {
    let request = SearchProfileIndexerSetRequest {
        indexer_instance_public_ids,
    };
    send_indexer_no_content(
        ctx,
        Method::PUT,
        &format!("/v1/indexers/search-profiles/{search_profile_public_id}{suffix}"),
        "/v1/indexers/search-profiles/{id}/indexers",
        &request,
    )
    .await?;
    println!("{success_message}");
    Ok(())
}

async fn handle_search_profile_tags(
    ctx: &AppContext,
    search_profile_public_id: Uuid,
    suffix: &str,
    success_message: &str,
    args: SearchProfileTagSetArgs,
) -> CliResult<()> {
    let tag_public_ids = (!args.tag_public_ids.is_empty()).then_some(args.tag_public_ids);
    let tag_keys = trim_string_values(args.tag_keys);
    let tag_keys = (!tag_keys.is_empty()).then_some(tag_keys);

    if tag_public_ids.is_none() && tag_keys.is_none() {
        return Err(CliError::validation(
            "at least one tag public id or tag key must be provided",
        ));
    }

    let request = SearchProfileTagSetRequest {
        tag_public_ids,
        tag_keys,
    };
    send_indexer_no_content(
        ctx,
        Method::PUT,
        &format!("/v1/indexers/search-profiles/{search_profile_public_id}{suffix}"),
        "/v1/indexers/search-profiles/{id}/tags",
        &request,
    )
    .await?;
    println!("{success_message}");
    Ok(())
}

pub(crate) async fn handle_tag_list(ctx: &AppContext, output: OutputFormat) -> CliResult<()> {
    let response: TagListResponse =
        get_indexer_resource(ctx, "/v1/indexers/tags", "/v1/indexers/tags").await?;
    render_tag_list(&response, output)
}

pub(crate) async fn handle_secret_list(ctx: &AppContext, output: OutputFormat) -> CliResult<()> {
    let response: SecretMetadataListResponse =
        get_indexer_resource(ctx, "/v1/indexers/secrets", "/v1/indexers/secrets").await?;
    render_secret_metadata_list(&response, output)
}

pub(crate) async fn handle_health_notification_hook_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerHealthNotificationHookListResponse = get_indexer_resource(
        ctx,
        "/v1/indexers/health-notifications",
        "/v1/indexers/health-notifications",
    )
    .await?;
    render_indexer_health_notification_hook_list(&response, output)
}

pub(crate) async fn handle_search_profile_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: SearchProfileListResponse = get_indexer_resource(
        ctx,
        "/v1/indexers/search-profiles",
        "/v1/indexers/search-profiles",
    )
    .await?;
    render_search_profile_list(&response, output)
}

pub(crate) async fn handle_policy_set_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: PolicySetListResponse =
        get_indexer_resource(ctx, "/v1/indexers/policies", "/v1/indexers/policies").await?;
    render_policy_set_list(&response, output)
}

pub(crate) async fn handle_routing_policy_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: RoutingPolicyListResponse = get_indexer_resource(
        ctx,
        "/v1/indexers/routing-policies",
        "/v1/indexers/routing-policies",
    )
    .await?;
    render_routing_policy_list(&response, output)
}

pub(crate) async fn handle_routing_policy_read(
    ctx: &AppContext,
    args: IndexerRoutingPolicyReadArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let response: RoutingPolicyDetailResponse = get_indexer_resource(
        ctx,
        &format!(
            "/v1/indexers/routing-policies/{}",
            args.routing_policy_public_id
        ),
        "/v1/indexers/routing-policies/{id}",
    )
    .await?;
    render_routing_policy_detail(&response, output)
}

pub(crate) async fn handle_rate_limit_policy_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: RateLimitPolicyListResponse =
        get_indexer_resource(ctx, "/v1/indexers/rate-limits", "/v1/indexers/rate-limits").await?;
    render_rate_limit_policy_list(&response, output)
}

pub(crate) async fn handle_indexer_instance_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerInstanceListResponse =
        get_indexer_resource(ctx, "/v1/indexers/instances", "/v1/indexers/instances").await?;
    render_indexer_instance_list(&response, output)
}

pub(crate) async fn handle_torznab_instance_list(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: TorznabInstanceListResponse = get_indexer_resource(
        ctx,
        "/v1/indexers/torznab-instances",
        "/v1/indexers/torznab-instances",
    )
    .await?;
    render_torznab_instance_list(&response, output)
}

pub(crate) async fn handle_indexer_backup_export(
    ctx: &AppContext,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerBackupExportResponse = get_indexer_resource(
        ctx,
        "/v1/indexers/backup/export",
        "/v1/indexers/backup/export",
    )
    .await?;
    render_indexer_backup_export(&response, output)
}

pub(crate) async fn handle_indexer_connectivity_read(
    ctx: &AppContext,
    args: IndexerInstanceReadArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerConnectivityProfileResponse = get_indexer_resource(
        ctx,
        &format!(
            "/v1/indexers/instances/{}/connectivity-profile",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/connectivity-profile",
    )
    .await?;
    render_indexer_connectivity_profile(&response, output)
}

pub(crate) async fn handle_indexer_reputation_read(
    ctx: &AppContext,
    args: IndexerInstanceReadArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerSourceReputationListResponse = get_indexer_resource(
        ctx,
        &format!(
            "/v1/indexers/instances/{}/reputation",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/reputation",
    )
    .await?;
    render_indexer_source_reputation_list(&response, output)
}

pub(crate) async fn handle_indexer_health_events_read(
    ctx: &AppContext,
    args: IndexerInstanceReadArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerHealthEventListResponse = get_indexer_resource(
        ctx,
        &format!(
            "/v1/indexers/instances/{}/health-events",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/health-events",
    )
    .await?;
    render_indexer_health_events(&response, output)
}

pub(crate) async fn handle_indexer_rss_read(
    ctx: &AppContext,
    args: IndexerInstanceReadArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let response: IndexerRssSubscriptionResponse = get_indexer_resource(
        ctx,
        &format!(
            "/v1/indexers/instances/{}/rss",
            args.indexer_instance_public_id
        ),
        "/v1/indexers/instances/{id}/rss",
    )
    .await?;
    render_indexer_rss_subscription(&response, output)
}

pub(crate) async fn handle_indexer_rss_items_read(
    ctx: &AppContext,
    args: IndexerInstanceRssItemsArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let mut path = format!(
        "/v1/indexers/instances/{}/rss/items",
        args.indexer_instance_public_id
    );
    if let Some(limit) = args.limit {
        let _ = write!(&mut path, "?limit={limit}");
    }

    let response: IndexerRssSeenItemsResponse =
        get_indexer_resource(ctx, &path, "/v1/indexers/instances/{id}/rss/items").await?;
    render_indexer_rss_seen_items(&response, output)
}

pub(crate) fn parse_import_job_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid import job id '{input}': {err}"))
}

pub(crate) fn parse_health_notification_hook_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid health notification hook id '{input}': {err}"))
}

pub(crate) async fn handle_indexer_instance_test_prepare(
    ctx: &AppContext,
    args: IndexerInstanceTestPrepareArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/instances/{}/test/prepare",
            args.indexer_instance_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/instances/test/prepare failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: IndexerInstanceTestPrepareResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse indexer test response: {err}"))
        })?;
        render_indexer_instance_test_prepare(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_indexer_instance_test_finalize(
    ctx: &AppContext,
    args: IndexerInstanceTestFinalizeArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = IndexerInstanceTestFinalizeRequest {
        ok: args.ok,
        error_class: trim_optional_text(args.error_class.as_deref()),
        error_code: trim_optional_text(args.error_code.as_deref()),
        detail: trim_optional_text(args.detail.as_deref()),
        result_count: args.result_count,
    };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/instances/{}/test/finalize",
            args.indexer_instance_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/instances/test/finalize failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: IndexerInstanceTestFinalizeResponse =
            response.json().await.map_err(|err| {
                CliError::failure(anyhow!("failed to parse indexer test response: {err}"))
            })?;
        render_indexer_instance_test_finalize(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_set_create(
    ctx: &AppContext,
    args: PolicySetCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }
    let scope = args.scope.trim();
    if scope.is_empty() {
        return Err(CliError::validation("scope must not be empty"));
    }

    let request = PolicySetCreateRequest {
        display_name: display_name.to_string(),
        scope: scope.to_string(),
        enabled: Some(args.enabled),
    };

    let url = ctx
        .base_url
        .join("/v1/indexers/policies")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!("request to /v1/indexers/policies failed: {err}"))
        })?;

    if response.status().is_success() {
        let payload: PolicySetResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse policy set response: {err}"))
        })?;
        render_policy_set_response(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_set_update(
    ctx: &AppContext,
    args: PolicySetUpdateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let display_name = args
        .display_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let request = PolicySetUpdateRequest { display_name };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/{}",
            args.policy_set_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .patch(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/update failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: PolicySetResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse policy set response: {err}"))
        })?;
        render_policy_set_response(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_set_enable(
    ctx: &AppContext,
    args: PolicySetEnableArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/{}/enable",
            args.policy_set_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/enable failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy set enabled (id: {})", args.policy_set_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_set_disable(
    ctx: &AppContext,
    args: PolicySetDisableArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/{}/disable",
            args.policy_set_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/disable failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy set disabled (id: {})", args.policy_set_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_set_reorder(
    ctx: &AppContext,
    args: PolicySetReorderArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = PolicySetReorderRequest {
        ordered_policy_set_public_ids: args.ordered_policy_set_public_ids,
    };

    let url = ctx
        .base_url
        .join("/v1/indexers/policies/reorder")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/reorder failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy sets reordered");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_rule_create(
    ctx: &AppContext,
    args: PolicyRuleCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;
    let request = build_policy_rule_request(&args)?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/{}/rules",
            args.policy_set_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/rules failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: PolicyRuleResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse policy rule response: {err}"))
        })?;
        render_policy_rule_response(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

fn build_policy_rule_request(args: &PolicyRuleCreateArgs) -> CliResult<PolicyRuleCreateRequest> {
    let rule_type = require_trimmed(&args.rule_type, "rule type must not be empty")?;
    let match_field = require_trimmed(&args.match_field, "match field must not be empty")?;
    let match_operator = require_trimmed(&args.match_operator, "match operator must not be empty")?;
    let action = require_trimmed(&args.action, "action must not be empty")?;
    let severity = require_trimmed(&args.severity, "severity must not be empty")?;
    let value_set_items = build_policy_rule_value_set_items(args);

    Ok(PolicyRuleCreateRequest {
        rule_type,
        match_field,
        match_operator,
        sort_order: args.sort_order,
        match_value_text: trim_optional_text(args.match_value_text.as_deref()),
        match_value_int: args.match_value_int,
        match_value_uuid: args.match_value_uuid,
        value_set_items,
        action,
        severity,
        is_case_insensitive: args.case_insensitive.then_some(true),
        rationale: trim_optional_text(args.rationale.as_deref()),
        expires_at: trim_optional_text(args.expires_at.as_deref()),
    })
}

fn build_policy_rule_value_set_items(
    args: &PolicyRuleCreateArgs,
) -> Option<Vec<PolicyRuleValueItemRequest>> {
    let mut value_set_items = Vec::new();
    for value in &args.value_set_text {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            value_set_items.push(PolicyRuleValueItemRequest {
                value_text: Some(trimmed.to_string()),
                value_int: None,
                value_bigint: None,
                value_uuid: None,
            });
        }
    }
    for value in &args.value_set_int {
        value_set_items.push(PolicyRuleValueItemRequest {
            value_text: None,
            value_int: Some(*value),
            value_bigint: None,
            value_uuid: None,
        });
    }
    for value in &args.value_set_bigint {
        value_set_items.push(PolicyRuleValueItemRequest {
            value_text: None,
            value_int: None,
            value_bigint: Some(*value),
            value_uuid: None,
        });
    }
    for value in &args.value_set_uuid {
        value_set_items.push(PolicyRuleValueItemRequest {
            value_text: None,
            value_int: None,
            value_bigint: None,
            value_uuid: Some(*value),
        });
    }

    if value_set_items.is_empty() {
        None
    } else {
        Some(value_set_items)
    }
}

fn require_trimmed(value: &str, message: &'static str) -> CliResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::validation(message));
    }
    Ok(trimmed.to_string())
}

fn trim_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trim_string_values(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn load_backup_snapshot(
    args: &BackupRestoreArgs,
) -> CliResult<revaer_api::models::IndexerBackupSnapshot> {
    let body = fs::read_to_string(&args.file).map_err(|err| {
        CliError::failure(anyhow!(
            "failed to read backup snapshot file {}: {err}",
            args.file.display()
        ))
    })?;
    serde_json::from_str(&body).map_err(|err| {
        CliError::failure(anyhow!(
            "failed to parse backup snapshot file {}: {err}",
            args.file.display()
        ))
    })
}

pub(crate) async fn handle_policy_rule_enable(
    ctx: &AppContext,
    args: PolicyRuleEnableArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/rules/{}/enable",
            args.policy_rule_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/rules/enable failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy rule enabled (id: {})", args.policy_rule_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_rule_disable(
    ctx: &AppContext,
    args: PolicyRuleDisableArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/rules/{}/disable",
            args.policy_rule_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/rules/disable failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy rule disabled (id: {})", args.policy_rule_public_id);
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_policy_rule_reorder(
    ctx: &AppContext,
    args: PolicyRuleReorderArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = PolicyRuleReorderRequest {
        ordered_policy_rule_public_ids: args.ordered_policy_rule_public_ids,
    };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/policies/{}/rules/reorder",
            args.policy_set_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/policies/rules/reorder failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!("Policy rules reordered");
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torznab_create(
    ctx: &AppContext,
    args: TorznabCreateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let display_name = args.display_name.trim();
    if display_name.is_empty() {
        return Err(CliError::validation("display name must not be empty"));
    }

    let request = TorznabInstanceCreateRequest {
        search_profile_public_id: args.search_profile_public_id,
        display_name: display_name.to_string(),
    };

    let url = ctx
        .base_url
        .join("/v1/indexers/torznab-instances")
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .post(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/torznab-instances failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: TorznabInstanceResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse torznab instance response: {err}"))
        })?;
        render_torznab_instance(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torznab_rotate(
    ctx: &AppContext,
    args: TorznabRotateArgs,
    output: OutputFormat,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/torznab-instances/{}/rotate",
            args.torznab_instance_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .patch(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/torznab-instances/rotate failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        let payload: TorznabInstanceResponse = response.json().await.map_err(|err| {
            CliError::failure(anyhow!("failed to parse torznab instance response: {err}"))
        })?;
        render_torznab_instance(&payload, output)
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torznab_set_state(
    ctx: &AppContext,
    args: TorznabSetStateArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let request = TorznabInstanceStateRequest {
        is_enabled: args.enabled,
    };

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/torznab-instances/{}/state",
            args.torznab_instance_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .put(url)
        .header(HEADER_API_KEY, creds.header_value())
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/torznab-instances/state failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!(
            "Torznab instance updated (id: {})",
            args.torznab_instance_public_id
        );
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) async fn handle_torznab_delete(
    ctx: &AppContext,
    args: TorznabDeleteArgs,
) -> CliResult<()> {
    let creds = ctx.api_key.as_ref().ok_or_else(|| {
        CliError::validation("API key is required (pass --api-key or set REVAER_API_KEY)")
    })?;

    let url = ctx
        .base_url
        .join(&format!(
            "/v1/indexers/torznab-instances/{}",
            args.torznab_instance_public_id
        ))
        .map_err(|err| CliError::failure(anyhow!("invalid base URL: {err}")))?;

    let response = ctx
        .client
        .delete(url)
        .header(HEADER_API_KEY, creds.header_value())
        .send()
        .await
        .map_err(|err| {
            CliError::failure(anyhow!(
                "request to /v1/indexers/torznab-instances failed: {err}"
            ))
        })?;

    if response.status().is_success() {
        println!(
            "Torznab instance deleted (id: {})",
            args.torznab_instance_public_id
        );
        Ok(())
    } else {
        Err(classify_problem(response).await)
    }
}

pub(crate) fn parse_torznab_instance_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid torznab instance id '{input}': {err}"))
}

pub(crate) fn parse_indexer_instance_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid indexer instance id '{input}': {err}"))
}

pub(crate) fn parse_policy_set_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid policy set id '{input}': {err}"))
}

pub(crate) fn parse_policy_rule_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid policy rule id '{input}': {err}"))
}

pub(crate) fn parse_search_profile_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid search profile id '{input}': {err}"))
}

pub(crate) fn parse_routing_policy_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid routing policy id '{input}': {err}"))
}

pub(crate) fn parse_rate_limit_policy_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid rate-limit policy id '{input}': {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use httpmock::prelude::*;
    use reqwest::Client;
    use revaer_api::models::{IndexerBackupSnapshot, IndexerBackupTagItem};
    use serde_json::json;

    use crate::client::ApiKeyCredential;

    fn context_with(server: &MockServer, api_key: Option<ApiKeyCredential>) -> Result<AppContext> {
        Ok(AppContext {
            client: Client::new(),
            base_url: server
                .base_url()
                .parse()
                .map_err(|_| anyhow!("valid URL"))?,
            api_key,
        })
    }

    fn context_with_key(server: &MockServer) -> Result<AppContext> {
        context_with(
            server,
            Some(ApiKeyCredential {
                key_id: "key".to_string(),
                secret: "secret".to_string(),
            }),
        )
    }

    fn sample_backup_snapshot() -> IndexerBackupSnapshot {
        IndexerBackupSnapshot {
            version: "1".to_string(),
            exported_at: Utc::now(),
            tags: vec![IndexerBackupTagItem {
                tag_key: "scene".to_string(),
                display_name: "Scene".to_string(),
            }],
            rate_limit_policies: Vec::new(),
            routing_policies: Vec::new(),
            indexer_instances: Vec::new(),
            secrets: Vec::new(),
        }
    }

    fn write_snapshot_file(body: &str) -> Result<std::path::PathBuf> {
        let path =
            std::env::temp_dir().join(format!("revaer-cli-indexer-backup-{}.json", Uuid::new_v4()));
        fs::write(&path, body)?;
        Ok(path)
    }

    #[tokio::test]
    async fn handle_tag_create_trims_and_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let tag_public_id = Uuid::new_v4();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/indexers/tags")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_key": "scene",
                    "display_name": "Scene Releases"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "tag_public_id": tag_public_id,
                    "tag_key": "scene",
                    "display_name": "Scene Releases"
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_tag_create(
            &ctx,
            TagCreateArgs {
                tag_key: "  scene  ".to_string(),
                display_name: "  Scene Releases  ".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_tag_update_posts_trimmed_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let tag_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path("/v1/indexers/tags")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_public_id": tag_public_id,
                    "tag_key": "scene",
                    "display_name": "Scene Updated"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "tag_public_id": tag_public_id,
                    "tag_key": "scene",
                    "display_name": "Scene Updated"
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_tag_update(
            &ctx,
            TagUpdateArgs {
                tag_public_id: Some(tag_public_id),
                tag_key: Some("  scene  ".to_string()),
                display_name: "  Scene Updated  ".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_tag_delete_posts_identifier_reference() -> Result<()> {
        let server = MockServer::start_async().await;
        let tag_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path("/v1/indexers/tags")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_public_id": tag_public_id,
                    "tag_key": "scene"
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_tag_delete(
            &ctx,
            TagDeleteArgs {
                tag_public_id: Some(tag_public_id),
                tag_key: Some("scene".to_string()),
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_tag_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let now = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/tags");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "tags": [{
                        "tag_public_id": Uuid::new_v4(),
                        "tag_key": "scene",
                        "display_name": "Scene",
                        "updated_at": now,
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_tag_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_secret_revoke_requires_api_key() -> Result<()> {
        let server = MockServer::start_async().await;
        let ctx = context_with(&server, None)?;
        let err = handle_secret_revoke(
            &ctx,
            SecretRevokeArgs {
                secret_public_id: Uuid::new_v4(),
            },
        )
        .await
        .err()
        .ok_or_else(|| anyhow!("missing api key should fail"))?;

        if !matches!(err, CliError::Validation(message) if message.contains("API key")) {
            return Err(anyhow!("expected API key validation error"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn handle_secret_create_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let secret_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/secrets")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "secret_type": "prowlarr_api_key",
                    "secret_value": "top-secret"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "secret_public_id": secret_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_secret_create(
            &ctx,
            SecretCreateArgs {
                secret_type: "  prowlarr_api_key  ".to_string(),
                secret_value: "top-secret".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_secret_rotate_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let secret_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path("/v1/indexers/secrets")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "secret_public_id": secret_public_id,
                    "secret_value": "rotated-value"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "secret_public_id": secret_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_secret_rotate(
            &ctx,
            SecretRotateArgs {
                secret_public_id,
                secret_value: "rotated-value".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_secret_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let created_at = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/secrets");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "secrets": [{
                        "secret_public_id": Uuid::new_v4(),
                        "secret_type": "api_key",
                        "is_revoked": false,
                        "created_at": created_at,
                        "binding_count": 2
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_secret_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_tracker_category_mapping_upsert_trims_optional_strings() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let indexer_instance_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/category-mappings/tracker")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "torznab_instance_public_id": torznab_instance_public_id,
                    "indexer_definition_upstream_slug": "prowlarr-demo",
                    "indexer_instance_public_id": indexer_instance_public_id,
                    "tracker_category": 2000,
                    "tracker_subcategory": 2070,
                    "torznab_cat_id": 5030,
                    "media_domain_key": "tv",
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_tracker_category_mapping_upsert(
            &ctx,
            TrackerCategoryMappingUpsertArgs {
                torznab_instance_public_id: Some(torznab_instance_public_id),
                indexer_definition_upstream_slug: Some("  prowlarr-demo  ".to_string()),
                indexer_instance_public_id: Some(indexer_instance_public_id),
                tracker_category: 2000,
                tracker_subcategory: Some(2070),
                torznab_cat_id: 5030,
                media_domain_key: Some("  tv  ".to_string()),
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_health_notification_hook_create_posts_trimmed_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let hook_public_id = Uuid::new_v4();
        let updated_at = Utc::now();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/health-notifications")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "channel": "webhook",
                    "display_name": "Ops Pager",
                    "status_threshold": "warning",
                    "webhook_url": "https://hooks.example.test/revaer"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "indexer_health_notification_hook_public_id": hook_public_id,
                    "channel": "webhook",
                    "display_name": "Ops Pager",
                    "status_threshold": "warning",
                    "webhook_url": "https://hooks.example.test/revaer",
                    "email": null,
                    "is_enabled": true,
                    "updated_at": updated_at
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_health_notification_hook_create(
            &ctx,
            HealthNotificationCreateArgs {
                channel: "  webhook  ".to_string(),
                display_name: "  Ops Pager  ".to_string(),
                status_threshold: "  warning  ".to_string(),
                webhook_url: Some("  https://hooks.example.test/revaer  ".to_string()),
                email: None,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_health_notification_hook_delete_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let hook_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path("/v1/indexers/health-notifications")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "indexer_health_notification_hook_public_id": hook_public_id
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_health_notification_hook_delete(
            &ctx,
            HealthNotificationDeleteArgs {
                indexer_health_notification_hook_public_id: hook_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_health_notification_hook_update_posts_trimmed_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let hook_public_id = Uuid::new_v4();
        let updated_at = Utc::now();
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path("/v1/indexers/health-notifications")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "indexer_health_notification_hook_public_id": hook_public_id,
                    "display_name": "Ops Pager",
                    "status_threshold": "critical",
                    "webhook_url": "https://hooks.example.test/critical",
                    "email": "ops@example.test",
                    "is_enabled": true
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "indexer_health_notification_hook_public_id": hook_public_id,
                    "channel": "webhook",
                    "display_name": "Ops Pager",
                    "status_threshold": "critical",
                    "webhook_url": "https://hooks.example.test/critical",
                    "email": "ops@example.test",
                    "is_enabled": true,
                    "updated_at": updated_at
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_health_notification_hook_update(
            &ctx,
            HealthNotificationUpdateArgs {
                indexer_health_notification_hook_public_id: hook_public_id,
                display_name: Some("  Ops Pager  ".to_string()),
                status_threshold: Some("  critical  ".to_string()),
                webhook_url: Some("  https://hooks.example.test/critical  ".to_string()),
                email: Some("  ops@example.test  ".to_string()),
                is_enabled: Some(true),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_health_notification_hook_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let updated_at = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/health-notifications");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "hooks": [{
                        "indexer_health_notification_hook_public_id": Uuid::new_v4(),
                        "channel": "webhook",
                        "display_name": "Ops Pager",
                        "status_threshold": "warning",
                        "webhook_url": "https://hooks.example.test/revaer",
                        "email": null,
                        "is_enabled": true,
                        "updated_at": updated_at
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_health_notification_hook_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_tracker_category_mapping_delete_posts_reference() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path("/v1/indexers/category-mappings/tracker")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "torznab_instance_public_id": torznab_instance_public_id,
                    "indexer_definition_upstream_slug": "prowlarr-demo",
                    "tracker_category": 2000,
                    "tracker_subcategory": 2070
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_tracker_category_mapping_delete(
            &ctx,
            TrackerCategoryMappingDeleteArgs {
                torznab_instance_public_id: Some(torznab_instance_public_id),
                indexer_definition_upstream_slug: Some("  prowlarr-demo  ".to_string()),
                indexer_instance_public_id: None,
                tracker_category: 2000,
                tracker_subcategory: Some(2070),
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_media_domain_mapping_delete_posts_trimmed_key() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path("/v1/indexers/category-mappings/media-domains")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "media_domain_key": "tv",
                    "torznab_cat_id": 5040
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_media_domain_mapping_delete(
            &ctx,
            MediaDomainMappingDeleteArgs {
                media_domain_key: "  tv  ".to_string(),
                torznab_cat_id: 5040,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn handle_media_domain_mapping_upsert_rejects_blank_key() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let server = MockServer::start();
        let ctx = context_with_key(&server)?;
        let err = runtime
            .block_on(handle_media_domain_mapping_upsert(
                &ctx,
                MediaDomainMappingUpsertArgs {
                    media_domain_key: "   ".to_string(),
                    torznab_cat_id: 5040,
                    is_primary: Some(true),
                },
            ))
            .err()
            .ok_or_else(|| anyhow!("blank media domain key should fail"))?;
        if !matches!(err, CliError::Validation(message) if message.contains("media domain key")) {
            return Err(anyhow!("expected media domain validation error"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn handle_routing_policy_create_posts_response() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/routing-policies")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Proxy",
                    "mode": "http_proxy",
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "routing_policy_public_id": routing_policy_public_id,
                    "display_name": "Proxy",
                    "mode": "http_proxy",
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_routing_policy_create(
            &ctx,
            RoutingPolicyCreateArgs {
                display_name: "  Proxy  ".to_string(),
                mode: "  http_proxy  ".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_routing_policy_set_param_trims_optional_text() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/routing-policies/{routing_policy_public_id}/params");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "param_key": "proxy_host",
                    "value_plain": "proxy.internal",
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_routing_policy_set_param(
            &ctx,
            RoutingPolicySetParamArgs {
                routing_policy_public_id,
                param_key: "  proxy_host  ".to_string(),
                value_plain: Some("  proxy.internal  ".to_string()),
                value_int: None,
                value_bool: None,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn handle_routing_policy_bind_secret_requires_parameter_key() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let server = MockServer::start();
        let ctx = context_with_key(&server)?;
        let err = runtime
            .block_on(handle_routing_policy_bind_secret(
                &ctx,
                RoutingPolicyBindSecretArgs {
                    routing_policy_public_id: Uuid::new_v4(),
                    param_key: "   ".to_string(),
                    secret_public_id: Uuid::new_v4(),
                },
            ))
            .err()
            .ok_or_else(|| anyhow!("blank parameter key should fail"))?;
        if !matches!(err, CliError::Validation(message) if message.contains("parameter key")) {
            return Err(anyhow!("expected parameter key validation error"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_assign_instance_posts_nullable_assignment() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/rate-limit");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({}));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_assign_instance(
            &ctx,
            RateLimitAssignInstanceArgs {
                indexer_instance_public_id,
                rate_limit_policy_public_id: None,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_policy_create_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let rate_limit_policy_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/rate-limits")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Primary",
                    "rpm": 60,
                    "burst": 10,
                    "concurrent": 4
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "rate_limit_policy_public_id": rate_limit_policy_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_policy_create(
            &ctx,
            RateLimitCreateArgs {
                display_name: "  Primary  ".to_string(),
                rpm: 60,
                burst: 10,
                concurrent: 4,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_policy_update_posts_partial_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let rate_limit_policy_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/rate-limits/{rate_limit_policy_public_id}");
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Primary Updated",
                    "rpm": 120
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "rate_limit_policy_public_id": rate_limit_policy_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_policy_update(
            &ctx,
            RateLimitUpdateArgs {
                rate_limit_policy_public_id,
                display_name: Some("  Primary Updated  ".to_string()),
                rpm: Some(120),
                burst: None,
                concurrent: None,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_policy_delete_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let rate_limit_policy_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/rate-limits/{rate_limit_policy_public_id}");
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(204);
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_policy_delete(
            &ctx,
            RateLimitDeleteArgs {
                rate_limit_policy_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_assign_routing_posts_nullable_assignment() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/routing-policies/{routing_policy_public_id}/rate-limit");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({}));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_assign_routing(
            &ctx,
            RateLimitAssignRoutingArgs {
                routing_policy_public_id,
                rate_limit_policy_public_id: None,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_media_domains_trims_values() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/media-domains");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "media_domain_keys": ["tv", "movie"]
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_media_domains(
            &ctx,
            SearchProfileSetMediaDomainsArgs {
                search_profile_public_id,
                media_domain_keys: vec![
                    " tv ".to_string(),
                    String::new(),
                    " movie ".to_string(),
                    "   ".to_string(),
                ],
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_create_posts_trimmed_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let user_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/search-profiles")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Movies",
                    "is_default": true,
                    "page_size": 50,
                    "default_media_domain_key": "movie",
                    "user_public_id": user_public_id
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "search_profile_public_id": search_profile_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_create(
            &ctx,
            SearchProfileCreateArgs {
                display_name: "  Movies  ".to_string(),
                is_default: true,
                page_size: Some(50),
                default_media_domain_key: Some("  movie  ".to_string()),
                user_public_id: Some(user_public_id),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/search-profiles");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "search_profiles": [{
                        "search_profile_public_id": search_profile_public_id,
                        "display_name": "Movies",
                        "is_default": true,
                        "page_size": 50,
                        "default_media_domain_key": "movie",
                        "media_domain_keys": ["movie", "tv"],
                        "policy_set_public_ids": [],
                        "policy_set_display_names": [],
                        "allow_indexer_public_ids": [],
                        "block_indexer_public_ids": [],
                        "allow_tag_keys": ["scene"],
                        "block_tag_keys": [],
                        "prefer_tag_keys": ["scene"]
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_update_posts_partial_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}");
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Movies Updated",
                    "page_size": 100
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "search_profile_public_id": search_profile_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_update(
            &ctx,
            SearchProfileUpdateArgs {
                search_profile_public_id,
                display_name: Some("  Movies Updated  ".to_string()),
                page_size: Some(100),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_default_domain_trims_optional_value() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let path =
            format!("/v1/indexers/search-profiles/{search_profile_public_id}/default-domain");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "default_media_domain_key": "tv"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "search_profile_public_id": search_profile_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_default_domain(
            &ctx,
            SearchProfileSetDefaultDomainArgs {
                search_profile_public_id,
                default_media_domain_key: Some("  tv  ".to_string()),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_default_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/default");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "page_size": 50
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "search_profile_public_id": search_profile_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_default(
            &ctx,
            SearchProfileSetDefaultArgs {
                search_profile_public_id,
                page_size: Some(50),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_add_policy_set_posts_reference() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let policy_set_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "policy_set_public_id": policy_set_public_id
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_add_policy_set(
            &ctx,
            SearchProfilePolicySetArgs {
                search_profile_public_id,
                policy_set_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_remove_policy_set_posts_reference() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let policy_set_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/policy-sets");
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "policy_set_public_id": policy_set_public_id
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_remove_policy_set(
            &ctx,
            SearchProfilePolicySetArgs {
                search_profile_public_id,
                policy_set_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_indexer_allow_posts_ids() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let expected_ids = ids.clone();
        let path =
            format!("/v1/indexers/search-profiles/{search_profile_public_id}/indexers/allow");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "indexer_instance_public_ids": expected_ids
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_indexer_allow(
            &ctx,
            SearchProfileIndexerSetArgs {
                search_profile_public_id,
                indexer_instance_public_ids: ids,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_indexer_block_posts_ids() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let expected_ids = ids.clone();
        let path =
            format!("/v1/indexers/search-profiles/{search_profile_public_id}/indexers/block");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "indexer_instance_public_ids": expected_ids
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_indexer_block(
            &ctx,
            SearchProfileIndexerSetArgs {
                search_profile_public_id,
                indexer_instance_public_ids: ids,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_tag_allow_posts_trimmed_keys() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/tags/allow");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_keys": ["scene", "anime"]
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_tag_allow(
            &ctx,
            SearchProfileTagSetArgs {
                search_profile_public_id,
                tag_public_ids: Vec::new(),
                tag_keys: vec![
                    "  scene  ".to_string(),
                    String::new(),
                    " anime ".to_string(),
                ],
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_tag_block_posts_public_ids() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let tag_public_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let expected_ids = tag_public_ids.clone();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/tags/block");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_public_ids": expected_ids
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_tag_block(
            &ctx,
            SearchProfileTagSetArgs {
                search_profile_public_id,
                tag_public_ids,
                tag_keys: Vec::new(),
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn handle_search_profile_set_tag_prefer_requires_identifiers() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let server = MockServer::start();
        let ctx = context_with_key(&server)?;
        let err = runtime
            .block_on(handle_search_profile_set_tag_prefer(
                &ctx,
                SearchProfileTagSetArgs {
                    search_profile_public_id: Uuid::new_v4(),
                    tag_public_ids: Vec::new(),
                    tag_keys: vec!["   ".to_string()],
                },
            ))
            .err()
            .ok_or_else(|| anyhow!("empty tag selection should fail"))?;
        if !matches!(err, CliError::Validation(message) if message.contains("tag public id")) {
            return Err(anyhow!("expected tag selection validation error"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn handle_search_profile_set_tag_prefer_posts_identifiers() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let tag_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/search-profiles/{search_profile_public_id}/tags/prefer");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "tag_public_ids": [tag_public_id],
                    "tag_keys": ["scene"]
                }));
            then.status(204);
        });

        let ctx = context_with_key(&server)?;
        handle_search_profile_set_tag_prefer(
            &ctx,
            SearchProfileTagSetArgs {
                search_profile_public_id,
                tag_public_ids: vec![tag_public_id],
                tag_keys: vec!["  scene  ".to_string()],
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_backup_restore_reads_snapshot_and_posts() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_backup_snapshot();
        let file = write_snapshot_file(&serde_json::to_string(&snapshot)?)?;
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/backup/restore")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "snapshot": snapshot
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "created_tag_count": 1,
                    "created_rate_limit_policy_count": 0,
                    "created_routing_policy_count": 0,
                    "created_indexer_instance_count": 0,
                    "unresolved_secret_bindings": []
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_backup_restore(
            &ctx,
            BackupRestoreArgs { file: file.clone() },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        fs::remove_file(file)?;
        Ok(())
    }

    #[test]
    fn handle_indexer_rss_mark_seen_requires_identifier() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let server = MockServer::start();
        let ctx = context_with_key(&server)?;
        let err = runtime
            .block_on(handle_indexer_rss_mark_seen(
                &ctx,
                IndexerRssMarkSeenArgs {
                    indexer_instance_public_id: Uuid::new_v4(),
                    item_guid: None,
                    infohash_v1: Some("   ".to_string()),
                    infohash_v2: Some(String::new()),
                    magnet_hash: None,
                },
                OutputFormat::Json,
            ))
            .err()
            .ok_or_else(|| anyhow!("missing identifiers should fail"))?;
        if !matches!(err, CliError::Validation(message) if message.contains("identifier")) {
            return Err(anyhow!("expected rss identifier validation error"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn handle_import_job_create_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let import_job_public_id = Uuid::new_v4();
        let target_search_profile = Uuid::new_v4();
        let target_torznab_instance = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/import-jobs")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "source": "prowlarr_backup",
                    "is_dry_run": true,
                    "target_search_profile_public_id": target_search_profile,
                    "target_torznab_instance_public_id": target_torznab_instance
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "import_job_public_id": import_job_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_import_job_create(
            &ctx,
            ImportJobCreateArgs {
                source: crate::cli::ImportSourceArg::ProwlarrBackup,
                dry_run: true,
                target_search_profile: Some(target_search_profile),
                target_torznab_instance: Some(target_torznab_instance),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_create_validates_and_posts() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/policies")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Movies",
                    "scope": "request",
                    "enabled": false
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "policy_set_public_id": policy_set_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_create(
            &ctx,
            PolicySetCreateArgs {
                display_name: "  Movies  ".to_string(),
                scope: "  request  ".to_string(),
                enabled: false,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_update_posts_trimmed_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/{policy_set_public_id}");
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "display_name": "Global policy"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "policy_set_public_id": policy_set_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_update(
            &ctx,
            PolicySetUpdateArgs {
                policy_set_public_id,
                display_name: Some("  Global policy  ".to_string()),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let policy_rule_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/policies");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "policy_sets": [{
                        "policy_set_public_id": policy_set_public_id,
                        "display_name": "Movies",
                        "scope": "profile",
                        "is_enabled": true,
                        "rules": [{
                            "policy_rule_public_id": policy_rule_public_id,
                            "rule_type": "block",
                            "match_field": "title",
                            "match_operator": "contains",
                            "sort_order": 10,
                            "match_value_text": "cam",
                            "action": "reject",
                            "severity": "high",
                            "is_case_insensitive": true,
                            "is_disabled": false
                        }]
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_reorder_posts_order() -> Result<()> {
        let server = MockServer::start_async().await;
        let ordered_policy_set_public_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let expected = ordered_policy_set_public_ids.clone();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/policies/reorder")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "ordered_policy_set_public_ids": expected
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_reorder(
            &ctx,
            PolicySetReorderArgs {
                ordered_policy_set_public_ids,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_enable_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/{policy_set_public_id}/enable");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_enable(
            &ctx,
            PolicySetEnableArgs {
                policy_set_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_set_disable_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/{policy_set_public_id}/disable");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_set_disable(
            &ctx,
            PolicySetDisableArgs {
                policy_set_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn build_policy_rule_request_trims_and_collects_value_set_items() -> Result<()> {
        let policy_set_public_id = Uuid::new_v4();
        let match_value_uuid = Uuid::new_v4();
        let value_uuid = Uuid::new_v4();
        let request = build_policy_rule_request(&PolicyRuleCreateArgs {
            policy_set_public_id,
            rule_type: "  block_title_regex  ".to_string(),
            match_field: "  title  ".to_string(),
            match_operator: "  regex  ".to_string(),
            action: "  drop_canonical  ".to_string(),
            severity: "  hard  ".to_string(),
            sort_order: 10,
            match_value_text: Some("  hdrip  ".to_string()),
            match_value_int: Some(4),
            match_value_uuid: Some(match_value_uuid),
            value_set_text: vec![" hdrip ".to_string(), " ".to_string()],
            value_set_int: vec![10],
            value_set_bigint: vec![20],
            value_set_uuid: vec![value_uuid],
            case_insensitive: true,
            rationale: Some("  cleanup  ".to_string()),
            expires_at: Some(" 2030-01-01T00:00:00Z ".to_string()),
        })?;

        assert_eq!(request.rule_type, "block_title_regex");
        assert_eq!(request.match_field, "title");
        assert_eq!(request.match_operator, "regex");
        assert_eq!(request.action, "drop_canonical");
        assert_eq!(request.severity, "hard");
        assert_eq!(request.match_value_text.as_deref(), Some("hdrip"));
        assert_eq!(request.match_value_int, Some(4));
        assert_eq!(request.match_value_uuid, Some(match_value_uuid));
        assert_eq!(request.is_case_insensitive, Some(true));
        assert_eq!(request.rationale.as_deref(), Some("cleanup"));
        assert_eq!(request.expires_at.as_deref(), Some("2030-01-01T00:00:00Z"));
        let value_set_items = request
            .value_set_items
            .ok_or_else(|| anyhow!("value set items should be present"))?;
        assert_eq!(value_set_items.len(), 4);
        assert!(
            value_set_items
                .iter()
                .any(|item| item.value_text.as_deref() == Some("hdrip"))
        );
        assert!(
            value_set_items
                .iter()
                .any(|item| item.value_int == Some(10))
        );
        assert!(
            value_set_items
                .iter()
                .any(|item| item.value_bigint == Some(20))
        );
        assert!(
            value_set_items
                .iter()
                .any(|item| item.value_uuid == Some(value_uuid))
        );
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_rule_create_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let policy_rule_public_id = Uuid::new_v4();
        let match_value_uuid = Uuid::new_v4();
        let value_set_uuid = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/{policy_set_public_id}/rules");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "rule_type": "tag",
                    "match_field": "tag_key",
                    "match_operator": "in_set",
                    "sort_order": 20,
                    "match_value_text": "scene",
                    "match_value_int": 7,
                    "match_value_uuid": match_value_uuid,
                    "value_set_items": [
                        {"value_text": "scene"},
                        {"value_int": 42},
                        {"value_bigint": 9_999_999_999_i64},
                        {"value_uuid": value_set_uuid}
                    ],
                    "action": "allow",
                    "severity": "info",
                    "is_case_insensitive": true,
                    "rationale": "keep releases",
                    "expires_at": "2026-04-05T00:00:00Z"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "policy_rule_public_id": policy_rule_public_id
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_policy_rule_create(
            &ctx,
            PolicyRuleCreateArgs {
                policy_set_public_id,
                rule_type: "  tag  ".to_string(),
                match_field: "  tag_key  ".to_string(),
                match_operator: "  in_set  ".to_string(),
                action: "  allow  ".to_string(),
                severity: "  info  ".to_string(),
                sort_order: 20,
                match_value_text: Some("  scene  ".to_string()),
                match_value_int: Some(7),
                match_value_uuid: Some(match_value_uuid),
                value_set_text: vec!["  scene  ".to_string(), "   ".to_string()],
                value_set_int: vec![42],
                value_set_bigint: vec![9_999_999_999],
                value_set_uuid: vec![value_set_uuid],
                case_insensitive: true,
                rationale: Some("  keep releases  ".to_string()),
                expires_at: Some(" 2026-04-05T00:00:00Z ".to_string()),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_rule_enable_posts() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_rule_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/rules/{policy_rule_public_id}/enable");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_rule_enable(
            &ctx,
            PolicyRuleEnableArgs {
                policy_rule_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_rule_disable_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_rule_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/policies/rules/{policy_rule_public_id}/disable");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_rule_disable(
            &ctx,
            PolicyRuleDisableArgs {
                policy_rule_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_policy_rule_reorder_posts_order() -> Result<()> {
        let server = MockServer::start_async().await;
        let policy_set_public_id = Uuid::new_v4();
        let ordered_policy_rule_public_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let path = format!("/v1/indexers/policies/{policy_set_public_id}/rules/reorder");
        let expected = ordered_policy_rule_public_ids.clone();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "ordered_policy_rule_public_ids": expected
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_policy_rule_reorder(
            &ctx,
            PolicyRuleReorderArgs {
                policy_set_public_id,
                ordered_policy_rule_public_ids,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_routing_policy_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/routing-policies");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "routing_policies": [{
                        "routing_policy_public_id": routing_policy_public_id,
                        "display_name": "Proxy",
                        "mode": "http_proxy",
                        "parameter_count": 2,
                        "secret_binding_count": 1
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_routing_policy_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_routing_policy_read_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let routing_policy_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/routing-policies/{routing_policy_public_id}");
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "routing_policy_public_id": routing_policy_public_id,
                    "display_name": "Proxy",
                    "mode": "http_proxy",
                    "parameters": [{
                        "param_key": "proxy_host",
                        "value_plain": "proxy.internal"
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_routing_policy_read(
            &ctx,
            IndexerRoutingPolicyReadArgs {
                routing_policy_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_rate_limit_policy_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let rate_limit_policy_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/rate-limits");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "rate_limit_policies": [{
                        "rate_limit_policy_public_id": rate_limit_policy_public_id,
                        "display_name": "Primary",
                        "requests_per_minute": 60,
                        "burst": 10,
                        "concurrent_requests": 4,
                        "is_system": false
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_rate_limit_policy_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_instance_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/instances");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "indexer_instances": [{
                        "indexer_instance_public_id": indexer_instance_public_id,
                        "upstream_slug": "demo-indexer",
                        "display_name": "Demo",
                        "instance_status": "enabled",
                        "rss_status": "enabled",
                        "automatic_search_status": "enabled",
                        "interactive_search_status": "enabled",
                        "priority": 50,
                        "connect_timeout_ms": 1000,
                        "read_timeout_ms": 5000,
                        "max_parallel_requests": 4,
                        "media_domain_keys": ["tv"],
                        "tag_keys": ["scene"],
                        "fields": [{
                            "field_name": "base_url",
                            "field_type": "text",
                            "value_plain": "https://indexer.example"
                        }]
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_instance_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_torznab_instance_list_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let search_profile_public_id = Uuid::new_v4();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/torznab-instances");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "torznab_instances": [{
                        "torznab_instance_public_id": torznab_instance_public_id,
                        "display_name": "Torznab Demo",
                        "is_enabled": true,
                        "search_profile_public_id": search_profile_public_id,
                        "search_profile_display_name": "Movies"
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_torznab_instance_list(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_backup_export_reads_snapshot() -> Result<()> {
        let server = MockServer::start_async().await;
        let snapshot = sample_backup_snapshot();
        server.mock(move |when, then| {
            when.method(GET).path("/v1/indexers/backup/export");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "snapshot": snapshot
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_backup_export(&ctx, OutputFormat::Json).await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_connectivity_read_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path =
            format!("/v1/indexers/instances/{indexer_instance_public_id}/connectivity-profile");
        let checked_at = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "profile_exists": true,
                    "status": "healthy",
                    "latency_p50_ms": 110,
                    "latency_p95_ms": 240,
                    "success_rate_1h": 0.97,
                    "success_rate_24h": 0.95,
                    "last_checked_at": checked_at
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_connectivity_read(
            &ctx,
            IndexerInstanceReadArgs {
                indexer_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_reputation_read_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/reputation");
        let now = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "items": [{
                        "window_key": "24h",
                        "window_start": now,
                        "request_success_rate": 0.95,
                        "acquisition_success_rate": 0.9,
                        "fake_rate": 0.02,
                        "dmca_rate": 0.01,
                        "request_count": 40,
                        "request_success_count": 38,
                        "acquisition_count": 10,
                        "acquisition_success_count": 9,
                        "min_samples": 5,
                        "computed_at": now
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_reputation_read(
            &ctx,
            IndexerInstanceReadArgs {
                indexer_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_health_events_read_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/health-events");
        let now = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "items": [{
                        "occurred_at": now,
                        "event_type": "identity_conflict",
                        "latency_ms": 503,
                        "http_status": 429,
                        "error_class": "cf_challenge",
                        "detail": "challenge observed"
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_health_events_read(
            &ctx,
            IndexerInstanceReadArgs {
                indexer_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_rss_set_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/rss");
        let now = Utc::now();
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "is_enabled": true,
                    "interval_seconds": 1800
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "indexer_instance_public_id": indexer_instance_public_id,
                    "instance_status": "enabled",
                    "rss_setting_status": "enabled",
                    "subscription_status": "enabled",
                    "interval_seconds": 1800,
                    "last_polled_at": now
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_rss_set(
            &ctx,
            IndexerRssSetArgs {
                indexer_instance_public_id,
                is_enabled: true,
                interval_seconds: Some(1800),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_rss_read_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/rss");
        let now = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "indexer_instance_public_id": indexer_instance_public_id,
                    "instance_status": "enabled",
                    "rss_setting_status": "enabled",
                    "subscription_status": "enabled",
                    "interval_seconds": 900,
                    "last_polled_at": now
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_rss_read(
            &ctx,
            IndexerInstanceReadArgs {
                indexer_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_rss_items_read_appends_limit() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let first_seen_at = Utc::now();
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/indexers/instances/00000000-0000-0000-0000-000000000000/rss/items")
                .query_param("limit", "5");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "items": [{
                        "item_guid": "guid-1",
                        "first_seen_at": first_seen_at
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_rss_items_read(
            &ctx,
            IndexerInstanceRssItemsArgs {
                indexer_instance_public_id: Uuid::nil(),
                limit: Some(5),
            },
            OutputFormat::Json,
        )
        .await?;
        let _ = indexer_instance_public_id;
        Ok(())
    }

    #[tokio::test]
    async fn handle_import_job_run_prowlarr_api_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let import_job_public_id = Uuid::new_v4();
        let secret_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-api");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "prowlarr_url": "http://localhost:9696",
                    "prowlarr_api_key_secret_public_id": secret_public_id
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_import_job_run_prowlarr_api(
            &ctx,
            ImportJobRunProwlarrApiArgs {
                import_job_public_id,
                prowlarr_url: "http://localhost:9696".to_string(),
                prowlarr_api_key_secret_public_id: secret_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_import_job_run_prowlarr_backup_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let import_job_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/import-jobs/{import_job_public_id}/run/prowlarr-backup");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "backup_blob_ref": "snapshot-ref"
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_import_job_run_prowlarr_backup(
            &ctx,
            ImportJobRunProwlarrBackupArgs {
                import_job_public_id,
                backup_blob_ref: "snapshot-ref".to_string(),
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_import_job_status_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let import_job_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/import-jobs/{import_job_public_id}/status");
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "status": "running",
                    "result_total": 3,
                    "result_imported_ready": 1,
                    "result_imported_needs_secret": 1,
                    "result_imported_test_failed": 0,
                    "result_unmapped_definition": 1,
                    "result_skipped_duplicate": 0
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_import_job_status(
            &ctx,
            ImportJobStatusArgs {
                import_job_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_import_job_results_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let import_job_public_id = Uuid::new_v4();
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/import-jobs/{import_job_public_id}/results");
        let created_at = Utc::now();
        server.mock(move |when, then| {
            when.method(GET).path(path.as_str());
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "results": [{
                        "prowlarr_identifier": "prowlarr-demo",
                        "upstream_slug": "demo-indexer",
                        "indexer_instance_public_id": indexer_instance_public_id,
                        "status": "imported_ready",
                        "detail": "ok",
                        "resolved_is_enabled": true,
                        "resolved_priority": 50,
                        "missing_secret_fields": 0,
                        "media_domain_keys": ["tv"],
                        "tag_keys": ["scene"],
                        "created_at": created_at
                    }]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_import_job_results(
            &ctx,
            ImportJobResultsArgs {
                import_job_public_id,
            },
            OutputFormat::Json,
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_instance_test_prepare_reads_resource() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let routing_policy_public_id = Uuid::new_v4();
        let secret_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/test/prepare");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "can_execute": true,
                    "engine": "torznab",
                    "routing_policy_public_id": routing_policy_public_id,
                    "connect_timeout_ms": 2000,
                    "read_timeout_ms": 8000,
                    "field_names": ["base_url", "api_key"],
                    "field_types": ["plain", "secret"],
                    "value_plain": ["https://indexer.example", null],
                    "secret_public_ids": [null, secret_public_id]
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_instance_test_prepare(
            &ctx,
            IndexerInstanceTestPrepareArgs {
                indexer_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_indexer_instance_test_finalize_posts_payload() -> Result<()> {
        let server = MockServer::start_async().await;
        let indexer_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/instances/{indexer_instance_public_id}/test/finalize");
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "ok": false,
                    "error_class": "network",
                    "error_code": "timeout",
                    "detail": "timed out",
                    "result_count": 0
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "ok": false,
                    "error_class": "network",
                    "error_code": "timeout",
                    "detail": "timed out",
                    "result_count": 0
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_indexer_instance_test_finalize(
            &ctx,
            IndexerInstanceTestFinalizeArgs {
                indexer_instance_public_id,
                ok: false,
                error_class: Some("network".to_string()),
                error_code: Some("timeout".to_string()),
                detail: Some("  timed out  ".to_string()),
                result_count: Some(0),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_torznab_create_posts_response() -> Result<()> {
        let server = MockServer::start_async().await;
        let search_profile_public_id = Uuid::new_v4();
        let torznab_instance_public_id = Uuid::new_v4();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/v1/indexers/torznab-instances")
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "search_profile_public_id": search_profile_public_id,
                    "display_name": "Operator feed"
                }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "torznab_instance_public_id": torznab_instance_public_id,
                    "api_key_plaintext": "torznab-secret"
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_torznab_create(
            &ctx,
            TorznabCreateArgs {
                search_profile_public_id,
                display_name: "  Operator feed  ".to_string(),
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_torznab_rotate_reads_response() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}/rotate");
        let mock = server.mock(move |when, then| {
            when.method(PATCH)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "torznab_instance_public_id": torznab_instance_public_id,
                    "api_key_plaintext": "rotated-secret"
                }));
        });

        let ctx = context_with_key(&server)?;
        handle_torznab_rotate(
            &ctx,
            TorznabRotateArgs {
                torznab_instance_public_id,
            },
            OutputFormat::Json,
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_torznab_set_state_posts_body() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}/state");
        let mock = server.mock(move |when, then| {
            when.method(PUT)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret")
                .json_body(json!({
                    "is_enabled": true
                }));
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_torznab_set_state(
            &ctx,
            TorznabSetStateArgs {
                torznab_instance_public_id,
                enabled: true,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[tokio::test]
    async fn handle_torznab_delete_posts_identifier() -> Result<()> {
        let server = MockServer::start_async().await;
        let torznab_instance_public_id = Uuid::new_v4();
        let path = format!("/v1/indexers/torznab-instances/{torznab_instance_public_id}");
        let mock = server.mock(move |when, then| {
            when.method(DELETE)
                .path(path.as_str())
                .header(HEADER_API_KEY, "key:secret");
            then.status(200);
        });

        let ctx = context_with_key(&server)?;
        handle_torznab_delete(
            &ctx,
            TorznabDeleteArgs {
                torznab_instance_public_id,
            },
        )
        .await?;

        mock.assert();
        Ok(())
    }

    #[test]
    fn load_backup_snapshot_reports_invalid_json() -> Result<()> {
        let file = write_snapshot_file("{ invalid json }")?;
        let err = load_backup_snapshot(&BackupRestoreArgs { file: file.clone() })
            .err()
            .ok_or_else(|| anyhow!("invalid json should fail"))?;
        fs::remove_file(file)?;
        if !matches!(err, CliError::Failure(message) if message.to_string().contains("failed to parse backup snapshot file"))
        {
            return Err(anyhow!("expected parse failure"));
        }
        Ok(())
    }

    #[test]
    fn identifier_parsers_reject_invalid_values() -> Result<()> {
        for parser in [
            parse_import_job_id as fn(&str) -> Result<Uuid, String>,
            parse_health_notification_hook_id,
            parse_torznab_instance_id,
            parse_indexer_instance_id,
            parse_policy_set_id,
            parse_policy_rule_id,
            parse_search_profile_id,
            parse_routing_policy_id,
            parse_rate_limit_policy_id,
        ] {
            if parser("not-a-uuid").is_ok() {
                return Err(anyhow!("invalid uuid should fail"));
            }
        }
        Ok(())
    }
}
