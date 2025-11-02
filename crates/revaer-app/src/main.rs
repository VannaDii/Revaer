#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::module_name_repetitions)]
#![allow(unexpected_cfgs)]
#![allow(clippy::multiple_crate_versions)]

//! Binary entrypoint that wires the Revaer services together and launches the
//! async orchestrators.

mod orchestrator;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use revaer_api::TorrentHandles;
use revaer_config::{AppMode, ConfigService};
use revaer_events::EventBus;
use revaer_telemetry::{GlobalContextGuard, LoggingConfig, Metrics};
use tracing::{error, info, warn};

#[cfg(feature = "libtorrent")]
use orchestrator::spawn_libtorrent_orchestrator;
#[cfg(feature = "libtorrent")]
use revaer_torrent_core::{TorrentInspector, TorrentWorkflow};
#[cfg(feature = "libtorrent")]
use revaer_torrent_libt::LibtorrentEngine;

/// Bootstraps the Revaer application and blocks until shutdown.
#[tokio::main]
async fn main() -> Result<()> {
    let logging = LoggingConfig::default();
    revaer_telemetry::init_logging(&logging)?;
    let _context = GlobalContextGuard::new("bootstrap");

    info!("Revaer application bootstrap starting");

    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL environment variable is required")?;

    let config = ConfigService::new(database_url)
        .await
        .context("failed to initialise configuration service")?;

    let (snapshot, watcher) = config
        .watch_settings(Duration::from_secs(5))
        .await
        .context("failed to initialise configuration watcher")?;

    let events = EventBus::new();
    let telemetry = Metrics::new().context("failed to initialise telemetry registry")?;

    #[cfg(feature = "libtorrent")]
    let (fsops_worker, config_task, torrent_handles) = {
        let (_engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &events,
            telemetry.clone(),
            snapshot.fs_policy.clone(),
            snapshot.engine_profile.clone(),
        )
        .await
        .context("failed to initialise torrent orchestrator")?;
        info!("Filesystem post-processing orchestrator ready");
        let workflow: Arc<dyn TorrentWorkflow> = orchestrator.clone();
        let inspector: Arc<dyn TorrentInspector> = orchestrator.clone();
        let handles = TorrentHandles::new(workflow, inspector);
        let config_task = spawn_config_watch_task(
            watcher,
            Arc::clone(&orchestrator),
            events.clone(),
            telemetry.clone(),
        );
        (worker, config_task, Some(handles))
    };

    #[cfg(not(feature = "libtorrent"))]
    let torrent_handles: Option<TorrentHandles> = {
        let _ = watcher;
        let _ = &snapshot.fs_policy;
        let _ = &snapshot.engine_profile;
        None
    };

    let api = revaer_api::ApiServer::new(
        config.clone(),
        events.clone(),
        torrent_handles,
        telemetry.clone(),
    )
    .context("failed to build API server")?;

    enforce_loopback_guard(
        &snapshot.app_profile.mode,
        snapshot.app_profile.bind_addr,
        &telemetry,
        &events,
    )?;

    let port = u16::try_from(snapshot.app_profile.http_port)
        .context("app_profile.http_port must fit inside a u16")?;
    ensure!(port != 0, "app_profile.http_port must be non-zero");

    let addr = SocketAddr::new(snapshot.app_profile.bind_addr, port);
    info!("Launching API listener on {}", addr);

    let serve_result = api.serve(addr).await;

    #[cfg(feature = "libtorrent")]
    {
        if !fsops_worker.is_finished() {
            fsops_worker.abort();
        }
        let _ = fsops_worker.await;

        if !config_task.is_finished() {
            config_task.abort();
        }
        let _ = config_task.await;
    }

    serve_result?;
    info!("API server shutdown complete");
    Ok(())
}

#[cfg(feature = "libtorrent")]
fn spawn_config_watch_task(
    mut watcher: revaer_config::ConfigWatcher,
    orchestrator: Arc<orchestrator::TorrentOrchestrator<LibtorrentEngine>>,
    events: EventBus,
    telemetry: Metrics,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        const APPLY_SLA: Duration = Duration::from_secs(2);
        let mut config_degraded = false;
        loop {
            let wait_started = Instant::now();
            match watcher.next().await {
                Ok(update) => {
                    telemetry.observe_config_watch_latency(wait_started.elapsed());
                    orchestrator
                        .update_fs_policy(update.fs_policy.clone())
                        .await;
                    let apply_started = Instant::now();
                    match orchestrator
                        .update_engine_profile(update.engine_profile.clone())
                        .await
                    {
                        Ok(()) => {
                            let apply_elapsed = apply_started.elapsed();
                            telemetry.observe_config_apply_latency(apply_elapsed);
                            let mut description = format!(
                                "watcher revision {} applied in {}ms",
                                update.revision,
                                apply_elapsed.as_millis()
                            );
                            if apply_elapsed > APPLY_SLA {
                                telemetry.inc_config_watch_slow();
                                warn!(
                                    revision = update.revision,
                                    elapsed_ms = apply_elapsed.as_millis(),
                                    "configuration update exceeded latency guard rail"
                                );
                                description = format!(
                                    "watcher revision {} applied after {}ms (exceeded guard rail)",
                                    update.revision,
                                    apply_elapsed.as_millis()
                                );
                                if !config_degraded {
                                    let _ = events.publish(revaer_events::Event::HealthChanged {
                                        degraded: vec!["config_watcher".to_string()],
                                    });
                                    config_degraded = true;
                                }
                            } else if config_degraded {
                                let _ = events.publish(revaer_events::Event::HealthChanged {
                                    degraded: vec![],
                                });
                                config_degraded = false;
                            }
                            let _ = events
                                .publish(revaer_events::Event::SettingsChanged { description });
                            info!(
                                revision = update.revision,
                                elapsed_ms = apply_elapsed.as_millis(),
                                "applied configuration update from watcher"
                            );
                        }
                        Err(err) => {
                            telemetry.inc_config_update_failure();
                            warn!(
                                error = %err,
                                revision = update.revision,
                                "failed to apply engine profile update from watcher"
                            );
                            let description = format!(
                                "failed to apply watcher revision {}: {}",
                                update.revision, err
                            );
                            let _ = events
                                .publish(revaer_events::Event::SettingsChanged { description });
                            if !config_degraded {
                                let _ = events.publish(revaer_events::Event::HealthChanged {
                                    degraded: vec!["config_watcher".to_string()],
                                });
                                config_degraded = true;
                            }
                        }
                    }
                }
                Err(err) => {
                    telemetry.inc_config_update_failure();
                    warn!(error = %err, "configuration watcher terminated");
                    if !config_degraded {
                        let _ = events.publish(revaer_events::Event::HealthChanged {
                            degraded: vec!["config_watcher".to_string()],
                        });
                    }
                    break;
                }
            }
        }
    })
}

fn enforce_loopback_guard(
    mode: &AppMode,
    bind_addr: IpAddr,
    telemetry: &Metrics,
    events: &EventBus,
) -> Result<()> {
    if matches!(mode, AppMode::Setup) && !bind_addr.is_loopback() {
        error!(
            bind_addr = %bind_addr,
            "refusing to bind setup mode API listener to non-loopback address"
        );
        telemetry.inc_guardrail_violation();
        let _ = events.publish(revaer_events::Event::HealthChanged {
            degraded: vec!["loopback_guard".to_string()],
        });
        bail!(
            "app_profile.bind_addr must remain on a loopback interface during setup mode (found {bind_addr})"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tokio::runtime::Runtime;

    #[test]
    fn loopback_guard_allows_loopback_address() {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new().expect("telemetry initialisation");
        enforce_loopback_guard(
            &AppMode::Setup,
            IpAddr::from_str("127.0.0.1").unwrap(),
            &metrics,
            &events,
        )
        .expect("loopback addresses should be allowed");

        enforce_loopback_guard(
            &AppMode::Active,
            IpAddr::from_str("192.168.1.1").unwrap(),
            &metrics,
            &events,
        )
        .expect("active mode should allow non-loopback addresses");
    }

    #[test]
    fn loopback_guard_rejects_public_interface_during_setup() {
        let events = EventBus::with_capacity(4);
        let metrics = Metrics::new().expect("telemetry initialisation");
        let mut stream = events.subscribe(None);
        let runtime = Runtime::new().expect("tokio runtime");

        let result = enforce_loopback_guard(
            &AppMode::Setup,
            IpAddr::from_str("192.168.10.20").unwrap(),
            &metrics,
            &events,
        );
        assert!(result.is_err(), "expected guard rail to reject address");

        let envelope = runtime
            .block_on(async { stream.next().await })
            .expect("health event emitted");
        assert!(matches!(
            envelope.event,
            revaer_events::Event::HealthChanged { .. }
        ));
    }
}
