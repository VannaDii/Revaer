//! Telemetry initialisation primitives and logging configuration.
//!
//! # Design
//! - Centralises logging setup (fmt or JSON) with a single entry point.
//! - Records the build SHA once to avoid inconsistencies across modules.
//! - Optionally installs an OpenTelemetry layer when the feature is enabled.

use std::borrow::Cow;

use anyhow::{Result, anyhow};
use once_cell::sync::OnceCell;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "otel")]
use opentelemetry::{global, trace::TracerProvider};
#[cfg(feature = "otel")]
use opentelemetry_sdk::trace as sdktrace;
#[cfg(feature = "otel")]
use tracing_opentelemetry::OpenTelemetryLayer;

/// Default logging target when `RUST_LOG` is not provided.
pub const DEFAULT_LOG_LEVEL: &str = "info";

static BUILD_SHA: OnceCell<String> = OnceCell::new();

/// Configure and install the global tracing subscriber.
///
/// # Errors
///
/// Returns an error if the tracing subscriber cannot be installed (for example,
/// because another subscriber has already been set globally).
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    init_logging_with_otel(config, None)?;
    Ok(())
}

/// Install the tracing subscriber with optional OpenTelemetry support.
///
/// Returns an `OpenTelemetryGuard` when the `otel` feature is enabled and the
/// provided configuration requests instrumentation; otherwise `None`.
///
/// # Errors
///
/// Returns an error if the tracing subscriber cannot be installed.
pub fn init_logging_with_otel<'a>(
    config: &LoggingConfig<'a>,
    otel: Option<&OpenTelemetryConfig<'a>>,
) -> Result<Option<OpenTelemetryGuard>> {
    BUILD_SHA
        .set(config.build_sha.to_string())
        .ok()
        .or(Some(()));

    #[cfg(feature = "otel")]
    if let Some(otel_config) = otel.filter(|cfg| cfg.enabled) {
        let telemetry = build_otel_layer(otel_config);
        let guard = install_with_otel_layer(config, telemetry)?;
        return Ok(Some(guard));
    }

    #[cfg(not(feature = "otel"))]
    if otel.is_some_and(|cfg| cfg.enabled) {
        eprintln!(
            "OpenTelemetry requested but the `revaer-telemetry` crate was built without the `otel` feature; continuing without exporter"
        );
    }

    install_fmt_subscriber(config)?;
    Ok(None)
}

/// Access the build SHA recorded during logging initialisation.
#[must_use]
pub fn build_sha() -> &'static str {
    BUILD_SHA.get().map_or("dev", String::as_str)
}

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig<'a> {
    /// Log level string (e.g., `info`, `debug`).
    pub level: &'a str,
    /// Output format selection for the tracing subscriber.
    pub format: LogFormat,
    /// Build identifier recorded in structured logs.
    pub build_sha: &'a str,
}

impl Default for LoggingConfig<'_> {
    fn default() -> Self {
        Self {
            level: DEFAULT_LOG_LEVEL,
            format: LogFormat::infer(),
            build_sha: build_sha(),
        }
    }
}

/// Available output formats for the logger.
#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    /// Emit logs as structured JSON objects.
    Json,
    /// Emit human-readable, pretty-printed logs.
    Pretty,
}

impl LogFormat {
    /// Choose a sensible default for the current build.
    #[must_use]
    pub const fn infer() -> Self {
        if cfg!(debug_assertions) {
            Self::Pretty
        } else {
            Self::Json
        }
    }
}

/// Minimal configuration describing when to enable OpenTelemetry instrumentation.
#[derive(Debug, Clone)]
pub struct OpenTelemetryConfig<'a> {
    /// Toggle flag; instrumentation is skipped when `false`.
    pub enabled: bool,
    /// Logical service name recorded in span resources.
    pub service_name: Cow<'a, str>,
    /// Optional endpoint placeholder for future exporters.
    pub endpoint: Option<Cow<'a, str>>,
}

#[cfg(feature = "otel")]
struct TelemetryLayer {
    layer: OpenTelemetryLayer<tracing_subscriber::registry::Registry, sdktrace::Tracer>,
    guard: OpenTelemetryGuard,
}

/// Guard returned when OpenTelemetry instrumentation is active.
pub struct OpenTelemetryGuard {
    #[cfg(feature = "otel")]
    provider: sdktrace::TracerProvider,
    #[cfg(not(feature = "otel"))]
    _private: (),
}

#[cfg(feature = "otel")]
impl Drop for OpenTelemetryGuard {
    fn drop(&mut self) {
        let _ = &self.provider;
        global::shutdown_tracer_provider();
    }
}

/// Convenience helper for deriving the log format from configuration maps.
#[must_use]
pub fn log_format_from_config(config: Option<&serde_json::Value>) -> Option<LogFormat> {
    config
        .and_then(|value| value.get("log_format"))
        .and_then(|value| value.as_str())
        .map(|value| match value {
            "json" => LogFormat::Json,
            "pretty" => LogFormat::Pretty,
            _ => LogFormat::infer(),
        })
}

fn install_fmt_subscriber(config: &LoggingConfig) -> Result<()> {
    match config.format {
        LogFormat::Json => tracing_subscriber::registry()
            .with(build_env_filter(config.level))
            .with(
                fmt::layer()
                    .json()
                    .with_target(false)
                    .with_thread_ids(false),
            )
            .try_init()
            .map_err(|err| anyhow!("failed to install tracing subscriber: {err}")),
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(build_env_filter(config.level))
            .with(fmt::layer().with_target(false).with_thread_ids(false))
            .try_init()
            .map_err(|err| anyhow!("failed to install tracing subscriber: {err}")),
    }
}

fn build_env_filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level))
}

#[cfg(feature = "otel")]
fn build_otel_layer(config: &OpenTelemetryConfig) -> TelemetryLayer {
    let provider = sdktrace::TracerProvider::builder().build();
    let tracer = provider.tracer(Cow::Owned(config.service_name.clone().into_owned()));
    global::set_tracer_provider(provider.clone());
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);
    TelemetryLayer {
        layer,
        guard: OpenTelemetryGuard { provider },
    }
}

#[cfg(feature = "otel")]
fn install_with_otel_layer(
    config: &LoggingConfig,
    telemetry: TelemetryLayer,
) -> Result<OpenTelemetryGuard> {
    let TelemetryLayer { layer, guard } = telemetry;
    match config.format {
        LogFormat::Json => tracing_subscriber::registry()
            .with(layer)
            .with(build_env_filter(config.level))
            .with(
                fmt::layer()
                    .json()
                    .with_target(false)
                    .with_thread_ids(false),
            )
            .try_init()
            .map_err(|err| anyhow!("failed to install tracing subscriber: {err}"))?,
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(layer)
            .with(build_env_filter(config.level))
            .with(fmt::layer().with_target(false).with_thread_ids(false))
            .try_init()
            .map_err(|err| anyhow!("failed to install tracing subscriber: {err}"))?,
    }
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn log_format_from_config_parses_variants() {
        let json_config = json!({"log_format": "json"});
        assert!(matches!(
            log_format_from_config(Some(&json_config)),
            Some(LogFormat::Json)
        ));

        let pretty_config = json!({"log_format": "pretty"});
        assert!(matches!(
            log_format_from_config(Some(&pretty_config)),
            Some(LogFormat::Pretty)
        ));

        let inferred = log_format_from_config(Some(&json!({"log_format": "unknown"})))
            .expect("expected format");
        match (LogFormat::infer(), inferred) {
            (LogFormat::Json, LogFormat::Json) | (LogFormat::Pretty, LogFormat::Pretty) => {}
            other => panic!("unexpected format mapping: {other:?}"),
        }

        assert!(log_format_from_config(None).is_none());
    }

    #[test]
    fn init_logging_installs_subscriber_once() {
        let config = LoggingConfig {
            level: "info",
            format: LogFormat::Pretty,
            build_sha: "dev",
        };
        let _ = init_logging(&config);
    }
}
