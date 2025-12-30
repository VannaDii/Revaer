//! Output renderers and formatting helpers for CLI commands.

use anyhow::anyhow;
use revaer_api::models::{TorrentDetail, TorrentListResponse, TorrentStateKind};
use revaer_config::ConfigSnapshot;
use revaer_torrent_core::FilePriority;

use crate::cli::OutputFormat;
use crate::client::{CliError, CliResult};

pub(crate) fn render_torrent_list(
    list: &TorrentListResponse,
    format: OutputFormat,
) -> CliResult<()> {
    match format {
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(list)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
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
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(detail)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
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
        OutputFormat::Json => {
            let text = serde_json::to_string_pretty(snapshot)
                .map_err(|err| CliError::failure(anyhow!("failed to format JSON: {err}")))?;
            println!("{text}");
        }
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
