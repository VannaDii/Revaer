//! Output renderers and formatting helpers for CLI commands.

use anyhow::anyhow;
use revaer_api::models::{
    ImportJobResponse, ImportJobResultsResponse, ImportJobStatusResponse,
    IndexerInstanceTestFinalizeResponse, IndexerInstanceTestPrepareResponse, PolicyRuleResponse,
    PolicySetResponse, TorrentDetail, TorrentListResponse, TorrentStateKind,
    TorznabInstanceResponse,
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
