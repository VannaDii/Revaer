//! Output renderers and formatting helpers for CLI commands.

use anyhow::anyhow;
use revaer_api::models::{
    ImportJobResponse, ImportJobResultsResponse, ImportJobStatusResponse,
    IndexerBackupExportResponse, IndexerBackupRestoreResponse, IndexerConnectivityProfileResponse,
    IndexerHealthEventListResponse, IndexerHealthNotificationHookListResponse,
    IndexerHealthNotificationHookResponse, IndexerInstanceListResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse,
    IndexerRssSeenItemsResponse, IndexerRssSeenMarkResponse, IndexerRssSubscriptionResponse,
    IndexerSourceReputationListResponse, PolicyRuleResponse, PolicySetListResponse,
    PolicySetResponse, RateLimitPolicyListResponse, RateLimitPolicyResponse,
    RoutingPolicyDetailResponse, RoutingPolicyListResponse, RoutingPolicyResponse,
    SearchProfileListResponse, SearchProfileResponse, SecretMetadataListResponse, SecretResponse,
    TagListResponse, TagResponse, TorrentDetail, TorrentListResponse, TorrentStateKind,
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

pub(crate) fn render_search_profile_response(
    _response: &SearchProfileResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            print_redacted_resource_json("search_profile", &["search_profile_public_id"])?;
        }
        OutputFormat::Table => {
            print_redacted_resource_table("search_profile", &["search_profile_public_id"]);
        }
    }
    Ok(())
}

pub(crate) fn render_routing_policy_response(
    _response: &RoutingPolicyResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "routing_policy",
            &["routing_policy_public_id", "display_name", "mode"],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "routing_policy",
                &["routing_policy_public_id", "display_name", "mode"],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_rate_limit_policy_response(
    _response: &RateLimitPolicyResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            print_redacted_resource_json("rate_limit_policy", &["rate_limit_policy_public_id"])?;
        }
        OutputFormat::Table => {
            print_redacted_resource_table("rate_limit_policy", &["rate_limit_policy_public_id"]);
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

pub(crate) fn render_tag_response(_response: &TagResponse, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            print_redacted_resource_json("tag", &["tag_public_id", "tag_key", "display_name"])?;
        }
        OutputFormat::Table => {
            print_redacted_resource_table("tag", &["tag_public_id", "tag_key", "display_name"]);
        }
    }
    Ok(())
}

pub(crate) fn render_tag_list(_list: &TagListResponse, format: OutputFormat) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            print_redacted_resource_json("tag_list", &["tags[].tag_public_id", "tags[].tag_key"])?;
        }
        OutputFormat::Table => {
            print_redacted_resource_table("tag_list", &["tags[].tag_public_id", "tags[].tag_key"]);
        }
    }
    Ok(())
}

pub(crate) fn render_secret_response(
    _response: &SecretResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            print_redacted_resource_json("secret", &["secret_public_id", "secret_plaintext"])?;
        }
        OutputFormat::Table => {
            print_redacted_resource_table("secret", &["secret_public_id", "secret_plaintext"]);
        }
    }
    Ok(())
}

pub(crate) fn render_secret_metadata_list(
    _list: &SecretMetadataListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "secret_metadata_list",
            &[
                "secrets[].secret_public_id",
                "secrets[].secret_type",
                "secrets[].binding_count",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "secret_metadata_list",
                &[
                    "secrets[].secret_public_id",
                    "secrets[].secret_type",
                    "secrets[].binding_count",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_health_notification_hook_response(
    _response: &IndexerHealthNotificationHookResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_health_notification_hook",
            &[
                "indexer_health_notification_hook_public_id",
                "channel",
                "display_name",
                "status_threshold",
                "is_enabled",
                "webhook_url",
                "email",
                "updated_at",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_health_notification_hook",
                &[
                    "indexer_health_notification_hook_public_id",
                    "channel",
                    "display_name",
                    "status_threshold",
                    "is_enabled",
                    "webhook_url",
                    "email",
                    "updated_at",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_health_notification_hook_list(
    _list: &IndexerHealthNotificationHookListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_health_notification_hook_list",
            &[
                "hooks[].indexer_health_notification_hook_public_id",
                "hooks[].channel",
                "hooks[].display_name",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_health_notification_hook_list",
                &[
                    "hooks[].indexer_health_notification_hook_public_id",
                    "hooks[].channel",
                    "hooks[].display_name",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_search_profile_list(
    _list: &SearchProfileListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "search_profile_list",
            &[
                "search_profiles[].search_profile_public_id",
                "search_profiles[].display_name",
                "search_profiles[].default_media_domain_key",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "search_profile_list",
                &[
                    "search_profiles[].search_profile_public_id",
                    "search_profiles[].display_name",
                    "search_profiles[].default_media_domain_key",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_policy_set_list(
    _list: &PolicySetListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "policy_set_list",
            &[
                "policy_sets[].policy_set_public_id",
                "policy_sets[].display_name",
                "policy_sets[].scope",
                "policy_sets[].rules",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "policy_set_list",
                &[
                    "policy_sets[].policy_set_public_id",
                    "policy_sets[].display_name",
                    "policy_sets[].scope",
                    "policy_sets[].rules",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_routing_policy_list(
    _list: &RoutingPolicyListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "routing_policy_list",
            &[
                "routing_policies[].routing_policy_public_id",
                "routing_policies[].display_name",
                "routing_policies[].mode",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "routing_policy_list",
                &[
                    "routing_policies[].routing_policy_public_id",
                    "routing_policies[].display_name",
                    "routing_policies[].mode",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_routing_policy_detail(
    _detail: &RoutingPolicyDetailResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "routing_policy_detail",
            &[
                "routing_policy_public_id",
                "display_name",
                "mode",
                "rate_limit_policy_public_id",
                "rate_limit_display_name",
                "parameters[].param_key",
                "parameters[].value_plain",
                "parameters[].value_int",
                "parameters[].value_bool",
                "parameters[].secret_binding_name",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "routing_policy_detail",
                &[
                    "routing_policy_public_id",
                    "display_name",
                    "mode",
                    "rate_limit_policy_public_id",
                    "rate_limit_display_name",
                    "parameters[].param_key",
                    "parameters[].value_plain",
                    "parameters[].value_int",
                    "parameters[].value_bool",
                    "parameters[].secret_binding_name",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_rate_limit_policy_list(
    _list: &RateLimitPolicyListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "rate_limit_policy_list",
            &[
                "rate_limit_policies[].rate_limit_policy_public_id",
                "rate_limit_policies[].display_name",
                "rate_limit_policies[].requests_per_minute",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "rate_limit_policy_list",
                &[
                    "rate_limit_policies[].rate_limit_policy_public_id",
                    "rate_limit_policies[].display_name",
                    "rate_limit_policies[].requests_per_minute",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_instance_list(
    _list: &IndexerInstanceListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_instance_list",
            &[
                "indexer_instances[].indexer_instance_public_id",
                "indexer_instances[].display_name",
                "indexer_instances[].upstream_slug",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_instance_list",
                &[
                    "indexer_instances[].indexer_instance_public_id",
                    "indexer_instances[].display_name",
                    "indexer_instances[].upstream_slug",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_torznab_instance_list(
    _list: &TorznabInstanceListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "torznab_instance_list",
            &[
                "torznab_instances[].torznab_instance_public_id",
                "torznab_instances[].display_name",
                "torznab_instances[].search_profile_display_name",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "torznab_instance_list",
                &[
                    "torznab_instances[].torznab_instance_public_id",
                    "torznab_instances[].display_name",
                    "torznab_instances[].search_profile_display_name",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_backup_export(
    _response: &IndexerBackupExportResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_backup_export",
            &[
                "snapshot.tags",
                "snapshot.rate_limit_policies",
                "snapshot.routing_policies",
                "snapshot.indexer_instances",
                "snapshot.secrets",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_backup_export",
                &[
                    "snapshot.tags",
                    "snapshot.rate_limit_policies",
                    "snapshot.routing_policies",
                    "snapshot.indexer_instances",
                    "snapshot.secrets",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_backup_restore(
    _response: &IndexerBackupRestoreResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_backup_restore",
            &[
                "created_tag_count",
                "created_rate_limit_policy_count",
                "created_routing_policy_count",
                "created_indexer_instance_count",
                "unresolved_secret_bindings",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_backup_restore",
                &[
                    "created_tag_count",
                    "created_rate_limit_policy_count",
                    "created_routing_policy_count",
                    "created_indexer_instance_count",
                    "unresolved_secret_bindings",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_connectivity_profile(
    _response: &IndexerConnectivityProfileResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_connectivity_profile",
            &[
                "profile_exists",
                "status",
                "error_class",
                "latency_p50_ms",
                "latency_p95_ms",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_connectivity_profile",
                &[
                    "profile_exists",
                    "status",
                    "error_class",
                    "latency_p50_ms",
                    "latency_p95_ms",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_source_reputation_list(
    _response: &IndexerSourceReputationListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_source_reputation_list",
            &[
                "items[].window_key",
                "items[].request_success_rate",
                "items[].acquisition_success_rate",
                "items[].fake_rate",
                "items[].request_count",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_source_reputation_list",
                &[
                    "items[].window_key",
                    "items[].request_success_rate",
                    "items[].acquisition_success_rate",
                    "items[].fake_rate",
                    "items[].request_count",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_health_events(
    _response: &IndexerHealthEventListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_health_events",
            &[
                "items[].occurred_at",
                "items[].event_type",
                "items[].error_class",
                "items[].detail",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_health_events",
                &[
                    "items[].occurred_at",
                    "items[].event_type",
                    "items[].error_class",
                    "items[].detail",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_rss_subscription(
    _response: &IndexerRssSubscriptionResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_rss_subscription",
            &[
                "indexer_instance_public_id",
                "instance_status",
                "rss_setting_status",
                "subscription_status",
                "interval_seconds",
                "last_polled_at",
                "next_poll_at",
                "backoff_seconds",
                "last_error_class",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_rss_subscription",
                &[
                    "indexer_instance_public_id",
                    "instance_status",
                    "rss_setting_status",
                    "subscription_status",
                    "interval_seconds",
                    "last_polled_at",
                    "next_poll_at",
                    "backoff_seconds",
                    "last_error_class",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_rss_seen_items(
    _response: &IndexerRssSeenItemsResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_rss_seen_items",
            &[
                "items[].first_seen_at",
                "items[].item_guid",
                "items[].infohash_v1",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_rss_seen_items",
                &[
                    "items[].first_seen_at",
                    "items[].item_guid",
                    "items[].infohash_v1",
                ],
            );
        }
    }
    Ok(())
}

pub(crate) fn render_indexer_rss_seen_mark(
    _response: &IndexerRssSeenMarkResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => print_redacted_resource_json(
            "indexer_rss_seen_mark",
            &[
                "inserted",
                "item.item_guid",
                "item.infohash_v1",
                "item.infohash_v2",
                "item.magnet_hash",
                "item.first_seen_at",
            ],
        )?,
        OutputFormat::Table => {
            print_redacted_resource_table(
                "indexer_rss_seen_mark",
                &[
                    "inserted",
                    "item.item_guid",
                    "item.infohash_v1",
                    "item.infohash_v2",
                    "item.magnet_hash",
                    "item.first_seen_at",
                ],
            );
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

fn print_redacted_resource_json(resource: &str, available_fields: &[&str]) -> CliResult<()> {
    print_json_value(&json!({
        "resource": resource,
        "payload_status": REDACTED_STATUS,
        "field_count": available_fields.len(),
    }))
}

fn print_redacted_resource_table(resource: &str, available_fields: &[&str]) {
    println!("resource: {resource}");
    println!("payload_status: {REDACTED_STATUS}");
    println!("field_count: {}", available_fields.len());
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
    use chrono::{DateTime, Utc};
    use revaer_api::models::{
        TorrentFileView, TorrentProgressView, TorrentRateLimit, TorrentRatesView,
        TorrentSelectionView, TorrentSettingsView, TorrentStateView, TorrentSummary,
    };
    use std::error::Error;
    use std::io;
    use uuid::Uuid;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    fn test_error(message: &'static str) -> Box<dyn Error> {
        Box::new(io::Error::other(message))
    }

    fn sample_timestamp() -> Result<DateTime<Utc>> {
        DateTime::<Utc>::from_timestamp(1_700_000_000, 0)
            .ok_or_else(|| test_error("timestamp invalid"))
    }

    fn sample_summary() -> Result<TorrentSummary> {
        let timestamp = sample_timestamp()?;
        Ok(TorrentSummary {
            id: Uuid::nil(),
            name: Some("demo".to_string()),
            state: TorrentStateView {
                kind: TorrentStateKind::Downloading,
                failure_message: None,
            },
            progress: TorrentProgressView {
                bytes_downloaded: 512,
                bytes_total: 1_024,
                percent_complete: 50.0,
                eta_seconds: Some(60),
            },
            rates: TorrentRatesView {
                download_bps: 2_048,
                upload_bps: 1_024,
                ratio: 1.5,
            },
            library_path: Some(".server_root/library".to_string()),
            download_dir: Some(".server_root/downloads".to_string()),
            sequential: true,
            tags: vec!["movies".to_string()],
            category: Some("movies".to_string()),
            trackers: vec!["udp://tracker.example".to_string()],
            rate_limit: Some(TorrentRateLimit {
                download_bps: Some(2_048),
                upload_bps: Some(1_024),
            }),
            connections_limit: Some(64),
            added_at: timestamp,
            completed_at: None,
            last_updated: timestamp,
        })
    }

    #[test]
    fn render_torrent_list_supports_table_and_json() -> Result<()> {
        let list = TorrentListResponse {
            torrents: vec![sample_summary()?],
            next: Some("cursor-1".to_string()),
        };

        render_torrent_list(&list, OutputFormat::Table)?;
        render_torrent_list(&list, OutputFormat::Json)?;
        Ok(())
    }

    #[test]
    fn render_torrent_detail_supports_table_and_json() -> Result<()> {
        let detail = TorrentDetail {
            summary: sample_summary()?,
            settings: Some(TorrentSettingsView {
                tags: vec!["movies".to_string()],
                category: Some("movies".to_string()),
                trackers: vec!["udp://tracker.example".to_string()],
                tracker_messages: std::collections::HashMap::new(),
                rate_limit: None,
                connections_limit: Some(32),
                download_dir: Some(".server_root/downloads".to_string()),
                comment: Some("comment".to_string()),
                source: Some("magnet".to_string()),
                private: Some(false),
                super_seeding: Some(false),
                seed_mode: Some(false),
                auto_managed: Some(true),
                queue_position: Some(1),
                seed_ratio_limit: Some(2.0),
                seed_time_limit: Some(3_600),
                selection: Some(TorrentSelectionView::default()),
                sequential: true,
                pex_enabled: Some(true),
                ..TorrentSettingsView::default()
            }),
            files: Some(vec![TorrentFileView {
                index: 0,
                path: "movie.mkv".to_string(),
                size_bytes: 1_024,
                bytes_completed: 512,
                priority: FilePriority::High,
                selected: true,
            }]),
        };

        render_torrent_detail(&detail, OutputFormat::Table)?;
        render_torrent_detail(&detail, OutputFormat::Json)?;
        Ok(())
    }

    #[test]
    fn format_helpers_cover_known_variants() {
        assert_eq!(format_priority(FilePriority::Skip), "skip");
        assert_eq!(format_priority(FilePriority::Normal), "normal");
        assert_eq!(state_to_str(TorrentStateKind::Queued), "queued");
        assert_eq!(state_to_str(TorrentStateKind::Seeding), "seeding");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2_048), "2.00 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.00 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
    }

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

    #[test]
    fn redacted_resource_json_omits_field_names() {
        let field_count = ["secret_public_id", "secret_plaintext"].len();
        let rendered = serde_json::to_string(&json!({
            "resource": "secret",
            "payload_status": REDACTED_STATUS,
            "field_count": field_count,
        }))
        .expect("redacted resource preview should serialize");
        assert!(rendered.contains("\"field_count\":2"));
        assert!(!rendered.contains("secret_public_id"));
        assert!(!rendered.contains("secret_plaintext"));
    }
}
