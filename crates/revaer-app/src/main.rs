mod orchestrator;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use revaer_api::TorrentHandles;
use revaer_config::{AppMode, ConfigService};
use revaer_events::EventBus;
use revaer_telemetry::{GlobalContextGuard, LoggingConfig};
use tracing::{error, info, warn};

#[cfg(feature = "libtorrent")]
use orchestrator::spawn_libtorrent_orchestrator;
#[cfg(feature = "libtorrent")]
use revaer_torrent_core::{TorrentInspector, TorrentWorkflow};

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

    #[cfg(feature = "libtorrent")]
    let (fsops_worker, config_task, torrent_handles) = {
        let (_engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &events,
            snapshot.fs_policy.clone(),
            snapshot.engine_profile.clone(),
        )
        .await
        .context("failed to initialise torrent orchestrator")?;
        info!("Filesystem post-processing orchestrator ready");
        let workflow: Arc<dyn TorrentWorkflow> = orchestrator.clone();
        let inspector: Arc<dyn TorrentInspector> = orchestrator.clone();
        let handles = TorrentHandles::new(workflow, inspector);
        let mut local_watcher = watcher;
        let orchestrator_for_updates = Arc::clone(&orchestrator);
        let config_task = tokio::spawn(async move {
            loop {
                match local_watcher.next().await {
                    Ok(update) => {
                        orchestrator_for_updates
                            .update_fs_policy(update.fs_policy.clone())
                            .await;
                        if let Err(err) = orchestrator_for_updates
                            .update_engine_profile(update.engine_profile.clone())
                            .await
                        {
                            warn!(
                                error = %err,
                                "failed to apply engine profile update from watcher"
                            );
                        } else {
                            info!(
                                revision = update.revision,
                                "applied configuration update from watcher"
                            );
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, "configuration watcher terminated");
                        break;
                    }
                }
            }
        });
        (worker, config_task, Some(handles))
    };

    #[cfg(not(feature = "libtorrent"))]
    let torrent_handles: Option<TorrentHandles> = {
        let _ = watcher;
        let _ = &snapshot.fs_policy;
        let _ = &snapshot.engine_profile;
        None
    };

    let api = revaer_api::ApiServer::new(config.clone(), events.clone(), torrent_handles)
        .context("failed to build API server")?;

    if snapshot.app_profile.mode == AppMode::Setup && !snapshot.app_profile.bind_addr.is_loopback()
    {
        error!(
            bind_addr = %snapshot.app_profile.bind_addr,
            "refusing to bind setup mode API listener to non-loopback address"
        );
        bail!(
            "app_profile.bind_addr must remain on a loopback interface during setup mode (found {})",
            snapshot.app_profile.bind_addr,
        );
    }

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
