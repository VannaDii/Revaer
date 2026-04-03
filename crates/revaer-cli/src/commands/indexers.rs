use std::fmt::Write as _;

use anyhow::anyhow;
use revaer_api::models::{
    ImportJobCreateRequest, ImportJobResponse, ImportJobResultsResponse,
    ImportJobRunProwlarrApiRequest, ImportJobRunProwlarrBackupRequest, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerConnectivityProfileResponse,
    IndexerHealthEventListResponse, IndexerInstanceListResponse,
    IndexerInstanceTestFinalizeRequest, IndexerInstanceTestFinalizeResponse,
    IndexerInstanceTestPrepareResponse, IndexerRssSeenItemsResponse,
    IndexerRssSubscriptionResponse, IndexerSourceReputationListResponse, PolicyRuleCreateRequest,
    PolicyRuleReorderRequest, PolicyRuleResponse, PolicyRuleValueItemRequest,
    PolicySetCreateRequest, PolicySetListResponse, PolicySetReorderRequest, PolicySetResponse,
    PolicySetUpdateRequest, RateLimitPolicyListResponse, RoutingPolicyDetailResponse,
    RoutingPolicyListResponse, SearchProfileListResponse, SecretMetadataListResponse,
    TagListResponse, TorznabInstanceCreateRequest, TorznabInstanceListResponse,
    TorznabInstanceResponse, TorznabInstanceStateRequest,
};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::cli::{
    ImportJobCreateArgs, ImportJobResultsArgs, ImportJobRunProwlarrApiArgs,
    ImportJobRunProwlarrBackupArgs, ImportJobStatusArgs, IndexerInstanceReadArgs,
    IndexerInstanceRssItemsArgs, IndexerInstanceTestFinalizeArgs, IndexerInstanceTestPrepareArgs,
    IndexerRoutingPolicyReadArgs, OutputFormat, PolicyRuleCreateArgs, PolicyRuleDisableArgs,
    PolicyRuleEnableArgs, PolicyRuleReorderArgs, PolicySetCreateArgs, PolicySetDisableArgs,
    PolicySetEnableArgs, PolicySetReorderArgs, PolicySetUpdateArgs, TorznabCreateArgs,
    TorznabDeleteArgs, TorznabRotateArgs, TorznabSetStateArgs,
};
use crate::client::{AppContext, CliError, CliResult, HEADER_API_KEY, classify_problem};
use crate::output::{
    render_import_job_results, render_import_job_status, render_import_job_summary,
    render_indexer_backup_export, render_indexer_connectivity_profile,
    render_indexer_health_events, render_indexer_instance_list,
    render_indexer_instance_test_finalize, render_indexer_instance_test_prepare,
    render_indexer_rss_seen_items, render_indexer_rss_subscription,
    render_indexer_source_reputation_list, render_policy_rule_response, render_policy_set_list,
    render_policy_set_response, render_rate_limit_policy_list, render_routing_policy_detail,
    render_routing_policy_list, render_search_profile_list, render_secret_metadata_list,
    render_tag_list, render_torznab_instance, render_torznab_instance_list,
};

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

pub(crate) fn parse_routing_policy_id(input: &str) -> Result<Uuid, String> {
    input
        .parse()
        .map_err(|err| format!("invalid routing policy id '{input}': {err}"))
}
