//! Output renderers and formatting helpers for CLI commands.

use anyhow::anyhow;
use revaer_api::models::{
    ImportJobResponse, ImportJobResultsResponse, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerConnectivityProfileResponse,
    IndexerHealthEventListResponse, IndexerInstanceListResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemsResponse, IndexerRssSubscriptionResponse,
    IndexerSourceReputationListResponse, PolicyRuleResponse, PolicySetListResponse,
    PolicySetResponse, RateLimitPolicyListResponse, RoutingPolicyDetailResponse,
    RoutingPolicyListResponse, SearchProfileListResponse, SecretMetadataListResponse,
    TagListResponse, TorrentDetail, TorrentListResponse, TorrentStateKind,
    TorznabInstanceListResponse, TorznabInstanceResponse,
};
use revaer_config::ConfigSnapshot;
use revaer_torrent_core::FilePriority;
use serde::Serialize;
use serde_json::json;

use crate::cli::OutputFormat;
use crate::client::{CliError, CliResult};

const REDACTED_VALUE: &str = "<redacted>";
const REDACTED_STATUS: &str = "redacted";

pub(crate) fn render_torrent_list(
    list: &TorrentListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!("{:<36} {:<18} {:>7} NAME", "ID", "STATE", "PROG");
            for summary in &list.torrents {
                let progress = format!("{:.1}%", summary.progress.percent_complete);
                let name = summary.name.as_deref().unwrap_or("<unnamed>");
                println!(
                    "{:<36} {:<18} {:>7} {}",
                    summary.id,
                    state_to_str(summary.state.kind),
                    progress,
                    name
                );
            }
            if let Some(next) = &list.next {
                println!("next cursor: {next}");
            }
        }
    }
    Ok(())
}

pub(crate) fn render_torrent_detail(detail: &TorrentDetail, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(detail)?,
        OutputFormat::Table => {
            let summary = &detail.summary;
            println!("id: {}", summary.id);
            if let Some(name) = &summary.name {
                println!("name: {name}");
            }
            println!("state: {}", state_to_str(summary.state.kind));
            if let Some(message) = &summary.state.failure_message {
                println!("reason: {message}");
            }
            println!(
                "progress: {:.1}% ({}/{})",
                summary.progress.percent_complete,
                format_bytes(summary.progress.bytes_downloaded),
                format_bytes(summary.progress.bytes_total)
            );
            println!(
                "rates: down {} / up {}",
                format_bytes(summary.rates.download_bps),
                format_bytes(summary.rates.upload_bps)
            );
            if let Some(path) = &summary.library_path {
                println!("library: {path}");
            }
            if !summary.tags.is_empty() {
                println!("tags: {}", summary.tags.join(", "));
            }
            if !summary.trackers.is_empty() {
                println!("trackers: {}", summary.trackers.join(", "));
            }
            println!("sequential: {}", summary.sequential);
            println!("added: {}", summary.added_at);
            println!("updated: {}", summary.last_updated);
            if let Some(files) = &detail.files {
                println!("files:");
                println!(
                    "  {:>5} {:>12} {:>12} {:<8} path",
                    "index", "size", "done", "priority"
                );
                for file in files {
                    println!(
                        "  {:>5} {:>12} {:>12} {:<8} {}",
                        file.index,
                        format_bytes(file.size_bytes),
                        format_bytes(file.bytes_completed),
                        format_priority(file.priority),
                        file.path
                    );
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn render_config_snapshot(
    snapshot: &ConfigSnapshot,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(snapshot)?,
        OutputFormat::Table => {
            println!("revision: {}", snapshot.revision);
            println!("mode: {}", snapshot.app_profile.mode.as_str());
            println!("instance: {}", snapshot.app_profile.instance_name);
            println!(
                "http bind: {}:{}",
                snapshot.app_profile.bind_addr, snapshot.app_profile.http_port
            );
            println!(
                "engine: {} (listen port: {:?})",
                snapshot.engine_profile.implementation, snapshot.engine_profile.listen_port
            );
            println!("download root: {}", snapshot.engine_profile.download_root);
            println!("resume dir: {}", snapshot.engine_profile.resume_dir);
            println!(
                "effective listen: {:?}, max_active: {:?}, dl: {:?}, ul: {:?}",
                snapshot.engine_profile_effective.network.listen_port,
                snapshot.engine_profile_effective.limits.max_active,
                snapshot.engine_profile_effective.limits.download_rate_limit,
                snapshot.engine_profile_effective.limits.upload_rate_limit
            );
            println!(
                "effective paths: download {} / resume {}",
                snapshot.engine_profile_effective.storage.download_root,
                snapshot.engine_profile_effective.storage.resume_dir
            );
            if !snapshot.engine_profile_effective.warnings.is_empty() {
                println!(
                    "engine warnings: {}",
                    snapshot.engine_profile_effective.warnings.join("; ")
                );
            }
            println!("library root: {}", snapshot.fs_policy.library_root);
        }
    }
    Ok(())
}

pub(crate) fn render_import_job_summary(
    job: &ImportJobResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(job)?,
        OutputFormat::Table => {
            println!("import_job_public_id: {}", job.import_job_public_id);
        }
    }
    Ok(())
}

pub(crate) fn render_import_job_status(
    status: &ImportJobStatusResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json_value(&json!({
            "status": status.status,
            "result_total": status.result_total,
            "result_imported_ready": status.result_imported_ready,
            "result_imported_test_failed": status.result_imported_test_failed,
            "result_unmapped_definition": status.result_unmapped_definition,
            "result_skipped_duplicate": status.result_skipped_duplicate
        }))?,
        OutputFormat::Table => {
            println!("status: {}", status.status);
            println!("total: {}", status.result_total);
            println!("imported_ready: {}", status.result_imported_ready);
            println!(
                "imported_test_failed: {}",
                status.result_imported_test_failed
            );
            println!("unmapped_definition: {}", status.result_unmapped_definition);
            println!("skipped_duplicate: {}", status.result_skipped_duplicate);
        }
    }
    Ok(())
}

pub(crate) fn render_import_job_results(
    results: &ImportJobResultsResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(results)?,
        OutputFormat::Table => {
            println!(
                "{:<24} {:<16} {:<36} {:<20} detail",
                "prowlarr_id", "status", "instance_id", "created_at"
            );
            for result in &results.results {
                let instance_id = result
                    .indexer_instance_public_id
                    .map_or_else(|| "-".to_string(), |id| id.to_string());
                let detail = result.detail.as_deref().unwrap_or("");
                println!(
                    "{:<24} {:<16} {:<36} {:<20} {}",
                    result.prowlarr_identifier,
                    result.status,
                    instance_id,
                    result.created_at,
                    detail
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_instance_test_prepare(
    response: &IndexerInstanceTestPrepareResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            let redacted = json!({
                "can_execute": response.can_execute,
                "error_class": response.error_class,
                "error_code": response.error_code,
                "detail": response.detail,
                "engine": response.engine,
                "routing_policy_public_id": response.routing_policy_public_id,
                "connect_timeout_ms": response.connect_timeout_ms,
                "read_timeout_ms": response.read_timeout_ms,
                "fields": build_prepare_field_preview(response)
            });
            print_json_value(&redacted)?;
        }
        OutputFormat::Table => {
            println!("can_execute: {}", response.can_execute);
            if let Some(error_class) = &response.error_class {
                println!("error_class: {error_class}");
            }
            if let Some(error_code) = &response.error_code {
                println!("error_code: {error_code}");
            }
            if let Some(detail) = &response.detail {
                println!("detail: {detail}");
            }
            println!("engine: {}", response.engine);
            println!(
                "routing_policy_public_id: {:?}",
                response.routing_policy_public_id
            );
            println!("connect_timeout_ms: {}", response.connect_timeout_ms);
            println!("read_timeout_ms: {}", response.read_timeout_ms);
            render_prepare_field_table(response);
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_instance_test_finalize(
    response: &IndexerInstanceTestFinalizeResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("ok: {}", response.ok);
            if let Some(error_class) = &response.error_class {
                println!("error_class: {error_class}");
            }
            if let Some(error_code) = &response.error_code {
                println!("error_code: {error_code}");
            }
            if let Some(detail) = &response.detail {
                println!("detail: {detail}");
            }
            if let Some(result_count) = response.result_count {
                println!("result_count: {result_count}");
            }
        }
    }
    Ok(())
}

pub(crate) fn render_policy_set_response(
    response: &PolicySetResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("policy_set_public_id: {}", response.policy_set_public_id);
        }
    }
    Ok(())
}

pub(crate) fn render_policy_rule_response(
    response: &PolicyRuleResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("policy_rule_public_id: {}", response.policy_rule_public_id);
        }
    }
    Ok(())
}

pub(crate) fn render_torznab_instance(
    instance: &TorznabInstanceResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json_value(&json!({
            "torznab_instance_public_id": instance.torznab_instance_public_id,
            "api_key_status": REDACTED_STATUS
        }))?,
        OutputFormat::Table => {
            println!(
                "torznab_instance_public_id: {}",
                instance.torznab_instance_public_id
            );
            println!("api_key: {REDACTED_VALUE}");
        }
    }
    Ok(())
}

pub(crate) fn render_tag_list(list: &TagListResponse, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!("{:<36} {:<24} DISPLAY NAME", "TAG ID", "KEY");
            for tag in &list.tags {
                println!(
                    "{:<36} {:<24} {}",
                    tag.tag_public_id, tag.tag_key, tag.display_name
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_secret_metadata_list(
    list: &SecretMetadataListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<14} {:<8} {:<8} ROTATED AT",
                "SECRET ID", "TYPE", "REVOKED", "BINDS"
            );
            for secret in &list.secrets {
                let rotated_at = secret
                    .rotated_at
                    .map_or_else(|| "-".to_string(), |value| value.to_rfc3339());
                println!(
                    "{:<36} {:<14} {:<8} {:<8} {}",
                    secret.secret_public_id,
                    secret.secret_type,
                    secret.is_revoked,
                    secret.binding_count,
                    rotated_at
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_search_profile_list(
    list: &SearchProfileListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<8} {:<8} DEFAULT DOMAIN",
                "PROFILE ID", "NAME", "DEFAULT", "PAGE"
            );
            for profile in &list.search_profiles {
                let page_size = profile
                    .page_size
                    .map_or_else(|| "-".to_string(), |value| value.to_string());
                let default_domain = profile.default_media_domain_key.as_deref().unwrap_or("-");
                println!(
                    "{:<36} {:<24} {:<8} {:<8} {}",
                    profile.search_profile_public_id,
                    profile.display_name,
                    profile.is_default,
                    page_size,
                    default_domain
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_policy_set_list(
    list: &PolicySetListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<10} {:<8} RULES",
                "POLICY SET ID", "NAME", "SCOPE", "ENABLED"
            );
            for policy_set in &list.policy_sets {
                println!(
                    "{:<36} {:<24} {:<10} {:<8} {}",
                    policy_set.policy_set_public_id,
                    policy_set.display_name,
                    policy_set.scope,
                    policy_set.is_enabled,
                    policy_set.rules.len()
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_routing_policy_list(
    list: &RoutingPolicyListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<14} {:<8} {:<8}",
                "ROUTING ID", "NAME", "MODE", "PARAMS", "SECRETS"
            );
            for policy in &list.routing_policies {
                println!(
                    "{:<36} {:<24} {:<14} {:<8} {:<8}",
                    policy.routing_policy_public_id,
                    policy.display_name,
                    policy.mode,
                    policy.parameter_count,
                    policy.secret_binding_count
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_routing_policy_detail(
    detail: &RoutingPolicyDetailResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(detail)?,
        OutputFormat::Table => {
            println!(
                "routing_policy_public_id: {}",
                detail.routing_policy_public_id
            );
            println!("display_name: {}", detail.display_name);
            println!("mode: {}", detail.mode);
            if let Some(rate_limit_id) = detail.rate_limit_policy_public_id {
                println!("rate_limit_policy_public_id: {rate_limit_id}");
            }
            if let Some(rate_limit_name) = &detail.rate_limit_display_name {
                println!("rate_limit_display_name: {rate_limit_name}");
            }
            println!("parameters:");
            println!(
                "  {:<24} {:<18} {:<10} {:<8} binding",
                "KEY", "VALUE", "INT", "BOOL"
            );
            for parameter in &detail.parameters {
                let value_plain = parameter.value_plain.as_deref().unwrap_or("-");
                let value_int = parameter
                    .value_int
                    .map_or_else(|| "-".to_string(), |value| value.to_string());
                let value_bool = parameter
                    .value_bool
                    .map_or_else(|| "-".to_string(), |value| value.to_string());
                let binding = parameter.secret_binding_name.as_deref().unwrap_or("-");
                println!(
                    "  {:<24} {:<18} {:<10} {:<8} {}",
                    parameter.param_key, value_plain, value_int, value_bool, binding
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_rate_limit_policy_list(
    list: &RateLimitPolicyListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<6} {:<6} {:<10} SYSTEM",
                "RATE LIMIT ID", "NAME", "RPM", "BURST", "CONCURRENT"
            );
            for policy in &list.rate_limit_policies {
                println!(
                    "{:<36} {:<24} {:<6} {:<6} {:<10} {}",
                    policy.rate_limit_policy_public_id,
                    policy.display_name,
                    policy.requests_per_minute,
                    policy.burst,
                    policy.concurrent_requests,
                    policy.is_system
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_instance_list(
    list: &IndexerInstanceListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<16} {:<8} {:<8} TAGS",
                "INSTANCE ID", "NAME", "UPSTREAM", "STATE", "RSS"
            );
            for instance in &list.indexer_instances {
                println!(
                    "{:<36} {:<24} {:<16} {:<8} {:<8} {}",
                    instance.indexer_instance_public_id,
                    instance.display_name,
                    instance.upstream_slug,
                    instance.instance_status,
                    instance.rss_status,
                    instance.tag_keys.join(",")
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_torznab_instance_list(
    list: &TorznabInstanceListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(list)?,
        OutputFormat::Table => {
            println!(
                "{:<36} {:<24} {:<8} PROFILE",
                "TORZNAB ID", "NAME", "ENABLED"
            );
            for instance in &list.torznab_instances {
                println!(
                    "{:<36} {:<24} {:<8} {}",
                    instance.torznab_instance_public_id,
                    instance.display_name,
                    instance.is_enabled,
                    instance.search_profile_display_name
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_backup_export(
    response: &IndexerBackupExportResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("tags: {}", response.snapshot.tags.len());
            println!(
                "rate_limit_policies: {}",
                response.snapshot.rate_limit_policies.len()
            );
            println!(
                "routing_policies: {}",
                response.snapshot.routing_policies.len()
            );
            println!(
                "indexer_instances: {}",
                response.snapshot.indexer_instances.len()
            );
            println!("secret_refs: {}", response.snapshot.secrets.len());
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_connectivity_profile(
    response: &IndexerConnectivityProfileResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("profile_exists: {}", response.profile_exists);
            if let Some(status) = &response.status {
                println!("status: {status}");
            }
            if let Some(error_class) = &response.error_class {
                println!("error_class: {error_class}");
            }
            if let Some(latency_p50_ms) = response.latency_p50_ms {
                println!("latency_p50_ms: {latency_p50_ms}");
            }
            if let Some(latency_p95_ms) = response.latency_p95_ms {
                println!("latency_p95_ms: {latency_p95_ms}");
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_source_reputation_list(
    response: &IndexerSourceReputationListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!(
                "{:<6} {:<12} {:<12} {:<10} REQUESTS",
                "WIN", "REQ OK", "ACQ OK", "FAKE"
            );
            for item in &response.items {
                println!(
                    "{:<6} {:<12.3} {:<12.3} {:<10.3} {}",
                    item.window_key,
                    item.request_success_rate,
                    item.acquisition_success_rate,
                    item.fake_rate,
                    item.request_count
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_health_events(
    response: &IndexerHealthEventListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!(
                "{:<26} {:<20} {:<10} DETAIL",
                "OCCURRED AT", "TYPE", "ERROR"
            );
            for item in &response.items {
                let error_class = item.error_class.as_deref().unwrap_or("-");
                let detail = item.detail.as_deref().unwrap_or("");
                println!(
                    "{:<26} {:<20} {:<10} {}",
                    item.occurred_at, item.event_type, error_class, detail
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_rss_subscription(
    response: &IndexerRssSubscriptionResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!(
                "indexer_instance_public_id: {}",
                response.indexer_instance_public_id
            );
            println!("instance_status: {}", response.instance_status);
            println!("rss_setting_status: {}", response.rss_setting_status);
            println!("subscription_status: {}", response.subscription_status);
            println!("interval_seconds: {}", response.interval_seconds);
            if let Some(last_polled_at) = response.last_polled_at {
                println!("last_polled_at: {last_polled_at}");
            }
            if let Some(next_poll_at) = response.next_poll_at {
                println!("next_poll_at: {next_poll_at}");
            }
            if let Some(backoff_seconds) = response.backoff_seconds {
                println!("backoff_seconds: {backoff_seconds}");
            }
            if let Some(last_error_class) = &response.last_error_class {
                println!("last_error_class: {last_error_class}");
            }
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_rss_seen_items(
    response: &IndexerRssSeenItemsResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_json(response)?,
        OutputFormat::Table => {
            println!("{:<26} {:<32} INFOHASH V1", "FIRST SEEN", "ITEM GUID");
            for item in &response.items {
                let item_guid = item.item_guid.as_deref().unwrap_or("-");
                let infohash_v1 = item.infohash_v1.as_deref().unwrap_or("-");
                println!(
                    "{:<26} {:<32} {}",
                    item.first_seen_at, item_guid, infohash_v1
                );
            }
        }
    }
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> CliResult<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
    println!("{text}");
    Ok(())
}

fn print_json_value(value: &serde_json::Value) -> CliResult<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
    println!("{text}");
    Ok(())
}

fn build_prepare_field_preview(
    response: &IndexerInstanceTestPrepareResponse,
) -> Option<Vec<serde_json::Value>> {
    response.field_names.as_ref().map(|field_names| {
        field_names
            .iter()
            .enumerate()
            .map(|(idx, name)| build_prepare_field_summary(response, idx, name))
            .collect()
    })
}

fn build_prepare_field_summary(
    response: &IndexerInstanceTestPrepareResponse,
    idx: usize,
    name: &str,
) -> serde_json::Value {
    json!({
        "name": name,
        "field_type": response
            .field_types
            .as_ref()
            .and_then(|items| items.get(idx))
            .map_or("-", String::as_str),
        "has_plain_value": response
            .value_plain
            .as_ref()
            .and_then(|items| items.get(idx))
            .is_some_and(Option::is_some),
        "has_int_value": response
            .value_int
            .as_ref()
            .and_then(|items| items.get(idx))
            .is_some_and(Option::is_some),
        "has_decimal_value": response
            .value_decimal
            .as_ref()
            .and_then(|items| items.get(idx))
            .is_some_and(Option::is_some),
        "has_bool_value": response
            .value_bool
            .as_ref()
            .and_then(|items| items.get(idx))
            .is_some_and(Option::is_some),
        "has_secret_binding": response
            .secret_public_ids
            .as_ref()
            .and_then(|items| items.get(idx))
            .is_some_and(Option::is_some)
    })
}

fn render_prepare_field_table(response: &IndexerInstanceTestPrepareResponse) {
    let Some(field_names) = &response.field_names else {
        return;
    };
    println!("fields:");
    for (idx, name) in field_names.iter().enumerate() {
        let field_type = response
            .field_types
            .as_ref()
            .and_then(|items| items.get(idx))
            .map_or("-", String::as_str);
        let plain_value = redacted_field_marker(
            response
                .value_plain
                .as_ref()
                .and_then(|items| items.get(idx))
                .is_some_and(Option::is_some),
            REDACTED_VALUE,
        );
        let int_value = redacted_field_marker(
            response
                .value_int
                .as_ref()
                .and_then(|items| items.get(idx))
                .is_some_and(Option::is_some),
            REDACTED_VALUE,
        );
        let decimal_value = redacted_field_marker(
            response
                .value_decimal
                .as_ref()
                .and_then(|items| items.get(idx))
                .is_some_and(Option::is_some),
            REDACTED_VALUE,
        );
        let bool_value = redacted_field_marker(
            response
                .value_bool
                .as_ref()
                .and_then(|items| items.get(idx))
                .is_some_and(Option::is_some),
            REDACTED_VALUE,
        );
        let credential_binding = redacted_field_marker(
            response
                .secret_public_ids
                .as_ref()
                .and_then(|items| items.get(idx))
                .is_some_and(Option::is_some),
            REDACTED_STATUS,
        );
        println!(
            "  {name} [{field_type}] plain={plain_value} int={int_value} decimal={decimal_value} bool={bool_value} credential_binding={credential_binding}"
        );
    }
}

const fn redacted_field_marker(is_present: bool, redacted_marker: &'static str) -> &'static str {
    if is_present { redacted_marker } else { "-" }
}

#[must_use]
pub(crate) const fn format_priority(priority: FilePriority) -> &'static str {
    match priority {
        FilePriority::Skip => "skip",
        FilePriority::Low => "low",
        FilePriority::Normal => "normal",
        FilePriority::High => "high",
    }
}

#[must_use]
pub(crate) const fn state_to_str(kind: TorrentStateKind) -> &'static str {
    match kind {
        TorrentStateKind::Queued => "queued",
        TorrentStateKind::FetchingMetadata => "fetching_metadata",
        TorrentStateKind::Downloading => "downloading",
        TorrentStateKind::Seeding => "seeding",
        TorrentStateKind::Completed => "completed",
        TorrentStateKind::Failed => "failed",
        TorrentStateKind::Stopped => "stopped",
    }
}

#[must_use]
pub(crate) fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes_to_f64(bytes);
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn bytes_to_f64(value: u64) -> f64 {
    let high = u32::try_from(value >> 32).unwrap_or(u32::MAX);
    let low = u32::try_from(value & 0xFFFF_FFFF).unwrap_or(u32::MAX);
    f64::from(high) * 4_294_967_296.0 + f64::from(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_prepare_json_marks_secret_presence_without_values() {
        let response = IndexerInstanceTestPrepareResponse {
            can_execute: true,
            error_class: None,
            error_code: None,
            detail: None,
            engine: "torznab".to_string(),
            routing_policy_public_id: None,
            connect_timeout_ms: 1_000,
            read_timeout_ms: 2_000,
            field_names: Some(vec!["cookie".to_string()]),
            field_types: Some(vec!["text".to_string()]),
            value_plain: Some(vec![Some("secret-cookie".to_string())]),
            value_int: Some(vec![None]),
            value_decimal: Some(vec![None]),
            value_bool: Some(vec![None]),
            secret_public_ids: Some(vec![Some(uuid::Uuid::from_u128(9))]),
        };

        let rendered = serde_json::to_string(&json!({
            "fields": build_prepare_field_preview(&response)
        }))
        .expect("prepare preview should serialize");
        assert!(rendered.contains("\"has_plain_value\":true"));
        assert!(rendered.contains("\"has_secret_binding\":true"));
        assert!(!rendered.contains("secret-cookie"));
        assert!(!rendered.contains("00000000-0000-0000-0000-000000000009"));
    }

    #[test]
    fn torznab_json_redacts_api_key() {
        let instance = TorznabInstanceResponse {
            torznab_instance_public_id: uuid::Uuid::from_u128(7),
            api_key_plaintext: "super-secret".to_string(),
        };

        let rendered = serde_json::to_string(&json!({
            "torznab_instance_public_id": instance.torznab_instance_public_id,
            "api_key_status": REDACTED_STATUS
        }))
        .expect("torznab preview should serialize");
        assert!(rendered.contains(REDACTED_STATUS));
        assert!(!rendered.contains("super-secret"));
        assert!(!rendered.contains("api_key_plaintext"));
    }
}
