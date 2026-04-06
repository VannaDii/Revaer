use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
#[cfg(feature = "libtorrent")]
use std::time::Instant;

use crate::error::{AppError, AppResult};
use crate::indexer_runtime::IndexerRuntime;
use crate::indexers::IndexerService;
use revaer_api::TorrentHandles;
use revaer_config::{AppMode, ConfigService, ConfigSnapshot, DbSessionConfig};
use revaer_events::EventBus;
use revaer_telemetry::{GlobalContextGuard, LoggingConfig, Metrics, OpenTelemetryConfig};
use tracing::{error, info, warn};

use revaer_runtime::RuntimeStore;

#[cfg(feature = "libtorrent")]
use crate::orchestrator::{
    EngineConfigurator, LibtorrentOrchestratorDeps, spawn_libtorrent_orchestrator,
};
#[cfg(feature = "libtorrent")]
use revaer_torrent_core::{TorrentEngine, TorrentInspector, TorrentWorkflow};

/// Dependencies required to bootstrap the Revaer application.
pub(crate) struct BootstrapDependencies {
    logging: LoggingConfig<'static>,
    otel_config: Option<OpenTelemetryConfig<'static>>,
    config: ConfigService,
    snapshot: ConfigSnapshot,
    watcher: revaer_config::ConfigWatcher,
    events: EventBus,
    telemetry: Metrics,
    #[cfg(feature = "libtorrent")]
    libtorrent: Option<LibtorrentOrchestratorDeps>,
}

impl BootstrapDependencies {
    /// Construct production dependencies from the environment for the binary entrypoint.
    pub(crate) async fn from_env() -> AppResult<Self> {
        let database_url = database_url_from_env()?;
        Self::from_database_url(database_url).await
    }

    pub(crate) async fn from_database_url(database_url: String) -> AppResult<Self> {
        let logging = LoggingConfig::default();
        let otel_config = load_otel_config_from_env();

        let secret_encryption_config = secret_session_from_env()?;
        let config = ConfigService::new_with_session(database_url, secret_encryption_config)
            .await
            .map_err(|err| AppError::config("config_service.new", err))?;

        let (snapshot, watcher) = config
            .watch_settings(Duration::from_secs(5))
            .await
            .map_err(|err| AppError::config("config_service.watch_settings", err))?;

        let events = EventBus::new();
        let telemetry =
            Metrics::new().map_err(|err| AppError::telemetry("telemetry.metrics", err))?;

        #[cfg(feature = "libtorrent")]
        let runtime = Some(
            RuntimeStore::new(config.pool().clone())
                .await
                .map_err(|err| AppError::runtime("runtime_store.new", err))?,
        );
        #[cfg(not(feature = "libtorrent"))]
        let _runtime: Option<RuntimeStore> = None;

        #[cfg(feature = "libtorrent")]
        let libtorrent = Some(LibtorrentOrchestratorDeps::new(
            &events, &telemetry, runtime,
        )?);

        Ok(Self {
            logging,
            otel_config,
            config,
            snapshot,
            watcher,
            events,
            telemetry,
            #[cfg(feature = "libtorrent")]
            libtorrent,
        })
    }
}

fn database_url_from_env() -> AppResult<String> {
    std::env::var("DATABASE_URL").map_err(|_| AppError::MissingEnv {
        name: "DATABASE_URL",
    })
}

/// Load the optional database session encryption configuration from the environment.
///
/// This function reads `REVAER_SECRET_KEY_ID` and `REVAER_SECRET_KEY` from the
/// process environment and, when both are present and valid, constructs a
/// [`DbSessionConfig`] used for envelope encryption of session data.
///
/// # When to set these variables
///
/// - Set both `REVAER_SECRET_KEY_ID` and `REVAER_SECRET_KEY` in deployments
///   where database session data must be encrypted at rest using envelope
///   encryption.
/// - Leave both unset if you do not want to enable this encryption mechanism;
///   in that case this function returns `Ok(None)` and no envelope encryption
///   key is configured.
///
/// The `REVAER_SECRET_KEY_ID` value is an opaque identifier that allows the
/// application to distinguish between different encryption keys (for example,
/// when performing key rotation). The `REVAER_SECRET_KEY` value is the actual
/// secret material used to encrypt and decrypt the encrypted payload.
///
/// # Consequences of misconfiguration
///
/// - If only one of `REVAER_SECRET_KEY_ID` or `REVAER_SECRET_KEY` is set, this
///   function returns an [`AppError::MissingEnv`] for the missing variable and
///   the application startup will fail.
/// - If either value is present but empty or only whitespace, this function
///   returns an [`AppError::InvalidConfig`] and the application startup will
///   fail.
/// - If these values are changed in a running deployment without migrating
///   existing data, previously encrypted sessions may become unreadable,
///   causing failures when attempting to decrypt stored data.
///
/// Callers should treat both variables as security-sensitive secrets and
/// manage them via a secure secret management system. Changes to either value
/// should be coordinated with any envelope encryption key-rotation or
/// data-migration process in order to avoid data loss.
fn secret_session_from_env() -> AppResult<Option<DbSessionConfig>> {
    let key_id = optional_env_var("REVAER_SECRET_KEY_ID")?;
    let secret_value = optional_env_var("REVAER_SECRET_KEY")?;

    secret_session_from_values(key_id.as_deref(), secret_value.as_deref())
}

fn optional_env_var(name: &'static str) -> AppResult<Option<String>> {
    optional_env_var_with(name, std::env::var)
}

fn optional_env_var_with(
    name: &'static str,
    getter: impl FnOnce(&'static str) -> Result<String, std::env::VarError>,
) -> AppResult<Option<String>> {
    match getter(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(AppError::InvalidConfig {
            field: name,
            reason: "env_not_unicode",
            value: None,
        }),
    }
}

fn secret_session_from_values(
    key_id: Option<&str>,
    secret_value: Option<&str>,
) -> AppResult<Option<DbSessionConfig>> {
    match (key_id, secret_value) {
        (None, None) => Ok(None),
        (Some(key_id), Some(secret_value)) => {
            // Maximum length: 128 bytes (as defined in `DbSessionConfig::SECRET_KEY_ID_MAX_LEN`).
            let trimmed_key_id = validate_trimmed_field(
                key_id,
                "REVAER_SECRET_KEY_ID",
                DbSessionConfig::SECRET_KEY_ID_MAX_LEN,
            )?;
            let trimmed_secret = validate_trimmed_field(
                secret_value,
                "REVAER_SECRET_KEY",
                DbSessionConfig::SECRET_KEY_MAX_LEN,
            )?;
            Ok(Some(DbSessionConfig::new(trimmed_key_id, trimmed_secret)))
        }
        (Some(_), None) => Err(AppError::MissingEnv {
            name: "REVAER_SECRET_KEY",
        }),
        (None, Some(_)) => Err(AppError::MissingEnv {
            name: "REVAER_SECRET_KEY_ID",
        }),
    }
}

fn validate_trimmed_field<'a>(
    value: &'a str,
    field: &'static str,
    max_len: usize,
) -> AppResult<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidConfig {
            field,
            reason: "empty",
            value: None,
        });
    }
    if trimmed.len() > max_len {
        return Err(AppError::InvalidConfig {
            field,
            reason: "too_long",
            value: None,
        });
    }
    Ok(trimmed)
}

/// Entry point for the Revaer application boot sequence.
///
/// # Errors
///
/// Returns an error if dependency construction or application startup fails.
pub async fn run_app() -> AppResult<()> {
    let dependencies = BootstrapDependencies::from_env().await?;
    Box::pin(run_app_with(dependencies)).await
}

/// Boot sequence using a provided database URL.
///
/// # Errors
///
/// Returns an error if dependency construction or application startup fails.
pub async fn run_app_with_database_url(database_url: String) -> AppResult<()> {
    let dependencies = BootstrapDependencies::from_database_url(database_url).await?;
    Box::pin(run_app_with(dependencies)).await
}

/// Boot sequence that relies entirely on injected dependencies to simplify testing.
pub(crate) async fn run_app_with(dependencies: BootstrapDependencies) -> AppResult<()> {
    let _otel_guard = init_bootstrap_logging(&dependencies)?;
    let _context = GlobalContextGuard::new("bootstrap");
    info!("Revaer application bootstrap starting");
    run_bootstrap_services(dependencies).await
}

fn init_bootstrap_logging(
    dependencies: &BootstrapDependencies,
) -> AppResult<Option<revaer_telemetry::OpenTelemetryGuard>> {
    let otel_ref = dependencies
        .otel_config
        .as_ref()
        .map(|cfg| cfg as &OpenTelemetryConfig);
    revaer_telemetry::init_logging_with_otel(&dependencies.logging, otel_ref)
        .map_err(|err| AppError::telemetry("telemetry.init", err))
}

async fn run_bootstrap_services(dependencies: BootstrapDependencies) -> AppResult<()> {
    let BootstrapDependencies {
        logging: _,
        otel_config: _,
        config,
        snapshot,
        watcher,
        events,
        telemetry,
        #[cfg(feature = "libtorrent")]
        libtorrent,
    } = dependencies;

    let addr = bootstrap_listener_addr(&snapshot.app_profile, &telemetry, &events)?;

    #[cfg(feature = "libtorrent")]
    let (fsops_worker, config_task, torrent_handles) = {
        let libtorrent = libtorrent.ok_or(AppError::MissingDependency { name: "libtorrent" })?;
        let (_engine, orchestrator, worker) = spawn_libtorrent_orchestrator(
            &events,
            snapshot.fs_policy.clone(),
            snapshot.engine_profile.clone(),
            libtorrent,
            Some(Arc::new(config.clone())),
        )
        .await?;
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

    let api = build_api_server(&config, &events, torrent_handles, telemetry.clone())?;
    let indexer_runtime_task =
        IndexerRuntime::new(Arc::new(config.clone()), telemetry.clone()).spawn();
    info!(addr = %addr, "Launching API listener");

    let serve_result = api.serve(addr).await;

    if !indexer_runtime_task.is_finished() {
        indexer_runtime_task.abort();
    }
    if let Err(err) = indexer_runtime_task.await {
        warn!(error = %err, "indexer runtime task join failed");
    }

    #[cfg(feature = "libtorrent")]
    {
        if !fsops_worker.is_finished() {
            fsops_worker.abort();
        }
        if let Err(err) = fsops_worker.await {
            warn!(error = %err, "fsops worker join failed");
        }

        if !config_task.is_finished() {
            config_task.abort();
        }
        if let Err(err) = config_task.await {
            warn!(error = %err, "config watcher task join failed");
        }
    }

    serve_result.map_err(|err| AppError::api_server("api_server.serve", err))?;
    info!("API server shutdown complete");
    Ok(())
}

fn bootstrap_listener_addr(
    app_profile: &revaer_config::AppProfile,
    telemetry: &Metrics,
    events: &EventBus,
) -> AppResult<SocketAddr> {
    enforce_loopback_guard(&app_profile.mode, app_profile.bind_addr, telemetry, events)?;
    let port = bootstrap_http_port(app_profile.http_port)?;
    Ok(SocketAddr::new(app_profile.bind_addr, port))
}

fn bootstrap_http_port(http_port: i32) -> AppResult<u16> {
    let port = u16::try_from(http_port).map_err(|_| AppError::InvalidConfig {
        field: "http_port",
        reason: "out_of_range",
        value: Some(http_port.to_string()),
    })?;
    if port == 0 {
        return Err(AppError::InvalidConfig {
            field: "http_port",
            reason: "zero",
            value: Some(http_port.to_string()),
        });
    }
    Ok(port)
}

fn build_api_server(
    config: &ConfigService,
    events: &EventBus,
    torrent_handles: Option<TorrentHandles>,
    telemetry: Metrics,
) -> AppResult<revaer_api::ApiServer> {
    let indexers = Arc::new(IndexerService::new(
        Arc::new(config.clone()),
        telemetry.clone(),
    ));
    revaer_api::ApiServer::new(
        config.clone(),
        indexers,
        events.clone(),
        torrent_handles,
        telemetry,
    )
    .map_err(|err| AppError::api_server("api_server.new", err))
}

fn load_otel_config_from_env() -> Option<OpenTelemetryConfig<'static>> {
    let enabled = env_flag("REVAER_ENABLE_OTEL");
    let service_name =
        std::env::var("REVAER_OTEL_SERVICE_NAME").unwrap_or_else(|_| "revaer-app".to_string());
    let endpoint = std::env::var("REVAER_OTEL_EXPORTER")
        .ok()
        .or_else(|| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok());
    otel_config_from_values(enabled, service_name, endpoint)
}

fn env_flag(name: &str) -> bool {
    env_flag_value(std::env::var(name).ok().as_deref())
}

fn env_flag_value(value: Option<&str>) -> bool {
    value.is_some_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn otel_config_from_values(
    enabled: bool,
    service_name: String,
    endpoint: Option<String>,
) -> Option<OpenTelemetryConfig<'static>> {
    if !enabled {
        return None;
    }
    Some(OpenTelemetryConfig {
        enabled: true,
        service_name: Cow::Owned(service_name),
        endpoint: endpoint.map(Cow::Owned),
    })
}

#[cfg(feature = "libtorrent")]
fn spawn_config_watch_task<E>(
    mut watcher: revaer_config::ConfigWatcher,
    orchestrator: Arc<crate::orchestrator::TorrentOrchestrator<E>>,
    events: EventBus,
    telemetry: Metrics,
) -> tokio::task::JoinHandle<()>
where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    tokio::spawn(async move {
        const APPLY_SLA: Duration = Duration::from_secs(2);
        let mut config_degraded = false;
        loop {
            let wait_started = Instant::now();
            match watcher.next().await {
                Ok(snapshot) => {
                    telemetry.observe_config_watch_latency(wait_started.elapsed());
                    apply_config_snapshot(
                        snapshot,
                        &orchestrator,
                        &events,
                        &telemetry,
                        &mut config_degraded,
                        APPLY_SLA,
                    )
                    .await;
                }
                Err(err) => {
                    telemetry.inc_config_update_failure();
                    warn!(error = %err, "configuration watcher terminated");
                    set_config_degraded(&events, &mut config_degraded, true);
                    break;
                }
            }
        }
    })
}

#[cfg(feature = "libtorrent")]
async fn apply_config_snapshot<E>(
    snapshot: revaer_config::ConfigSnapshot,
    orchestrator: &crate::orchestrator::TorrentOrchestrator<E>,
    events: &EventBus,
    telemetry: &Metrics,
    config_degraded: &mut bool,
    apply_sla: Duration,
) where
    E: TorrentEngine + EngineConfigurator + 'static,
{
    orchestrator
        .update_fs_policy(snapshot.fs_policy.clone())
        .await;
    let apply_started = Instant::now();
    match orchestrator
        .update_engine_profile(snapshot.engine_profile.clone())
        .await
    {
        Ok(()) => {
            let apply_elapsed = apply_started.elapsed();
            telemetry.observe_config_apply_latency(apply_elapsed);
            let mut description = format!(
                "watcher revision {} applied in {}ms",
                snapshot.revision,
                apply_elapsed.as_millis()
            );
            if apply_elapsed > apply_sla {
                telemetry.inc_config_watch_slow();
                warn!(
                    revision = snapshot.revision,
                    elapsed_ms = apply_elapsed.as_millis(),
                    "configuration update exceeded latency guard rail"
                );
                description = format!(
                    "watcher revision {} applied after {}ms (exceeded guard rail)",
                    snapshot.revision,
                    apply_elapsed.as_millis()
                );
                set_config_degraded(events, config_degraded, true);
            } else {
                set_config_degraded(events, config_degraded, false);
            }
            publish_event(
                events,
                revaer_events::Event::SettingsChanged { description },
            );
            info!(
                revision = snapshot.revision,
                elapsed_ms = apply_elapsed.as_millis(),
                "applied configuration update from watcher"
            );
        }
        Err(err) => {
            telemetry.inc_config_update_failure();
            warn!(
                error = %err,
                revision = snapshot.revision,
                "failed to apply engine profile update from watcher"
            );
            let description = format!(
                "failed to apply watcher revision {}: {}",
                snapshot.revision, err
            );
            publish_event(
                events,
                revaer_events::Event::SettingsChanged { description },
            );
            set_config_degraded(events, config_degraded, true);
        }
    }
}

#[cfg(feature = "libtorrent")]
fn set_config_degraded(events: &EventBus, config_degraded: &mut bool, degraded: bool) {
    if *config_degraded == degraded {
        return;
    }
    let degraded_list = if degraded {
        vec!["config_watcher".to_string()]
    } else {
        Vec::new()
    };
    publish_event(
        events,
        revaer_events::Event::HealthChanged {
            degraded: degraded_list,
        },
    );
    *config_degraded = degraded;
}

fn enforce_loopback_guard(
    mode: &AppMode,
    bind_addr: IpAddr,
    telemetry: &Metrics,
    events: &EventBus,
) -> AppResult<()> {
    if matches!(mode, AppMode::Setup) && !bind_addr.is_loopback() {
        error!(
            bind_addr = %bind_addr,
            "refusing to bind setup mode API listener to non-loopback address"
        );
        telemetry.inc_guardrail_violation();
        publish_event(
            events,
            revaer_events::Event::HealthChanged {
                degraded: vec!["loopback_guard".to_string()],
            },
        );
        return Err(AppError::InvalidConfig {
            field: "bind_addr",
            reason: "non_loopback_in_setup",
            value: Some(bind_addr.to_string()),
        });
    }
    Ok(())
}

fn publish_event(events: &EventBus, event: revaer_events::Event) {
    if let Err(error) = events.publish(event) {
        tracing::warn!(
            event_id = error.event_id(),
            event_kind = error.event_kind(),
            error = %error,
            "failed to publish event"
        );
    }
}

#[cfg(test)]
#[path = "bootstrap/tests.rs"]
mod tests;
