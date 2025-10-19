//! Telemetry primitives shared across the Revaer workspace.
//!
//! This crate centralises logging, metrics, and cross-service tracing helpers so the
//! application and delivery surfaces can adopt a consistent observability story.

use std::convert::TryFrom;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use once_cell::sync::OnceCell;
use prometheus::{Encoder, IntCounter, IntCounterVec, IntGauge, Opts, Registry, TextEncoder};
use serde::Serialize;
use serde_json::Value;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tracing::{Span, span::Entered};
use tracing_subscriber::{EnvFilter, fmt};

/// Default logging target when `RUST_LOG` is not provided.
const DEFAULT_LOG_LEVEL: &str = "info";

static BUILD_SHA: OnceCell<String> = OnceCell::new();

/// Configure and install the global tracing subscriber.
///
/// # Errors
///
/// Returns an error if the tracing subscriber cannot be installed (for example,
/// because another subscriber has already been set globally).
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    BUILD_SHA
        .set(config.build_sha.to_string())
        .ok()
        .or(Some(()));

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level));

    let install = |format: LogFormat| {
        let builder = fmt::fmt()
            .with_env_filter(env_filter.clone())
            .with_target(false)
            .with_thread_ids(false);

        match format {
            LogFormat::Json => builder.json().try_init(),
            LogFormat::Pretty => builder.pretty().try_init(),
        }
    };

    install(config.format).map_err(|err| anyhow!("failed to install tracing subscriber: {err}"))?;

    Ok(())
}

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig<'a> {
    pub level: &'a str,
    pub format: LogFormat,
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
    Json,
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

/// Guard that keeps the application-level span entered for the lifetime of the process.
pub struct GlobalContextGuard {
    _guard: Entered<'static>,
}

impl GlobalContextGuard {
    #[must_use]
    pub fn new(mode: impl Into<String>) -> Self {
        let mode = mode.into();
        let span: &'static Span = Box::leak(Box::new(
            tracing::info_span!("app", mode = %mode, build_sha = %build_sha()),
        ));
        let guard = span.enter();
        Self { _guard: guard }
    }
}

/// Record the current application mode on the active span.
pub fn record_app_mode(mode: &str) {
    Span::current().record("mode", tracing::field::display(mode));
}

/// Access the build SHA recorded during logging initialisation.
#[must_use]
pub fn build_sha() -> &'static str {
    BUILD_SHA.get().map_or("dev", String::as_str)
}

/// Capture request context for downstream telemetry.
pub fn set_request_context(span: &Span, request_id: impl Into<String>, route: impl Into<String>) {
    let request_id = request_id.into();
    let route = route.into();
    span.record("request_id", tracing::field::display(&request_id));
    span.record("route", tracing::field::display(&route));
}

/// Retrieve the request identifier from the current span, if one is set.
#[must_use]
pub fn current_request_id() -> Option<String> {
    ACTIVE_REQUEST_CONTEXT
        .try_with(|ctx| ctx.request_id.as_ref().to_string())
        .ok()
}

/// Retrieve the matched route from the current span, if one is set.
#[must_use]
pub fn current_route() -> Option<String> {
    ACTIVE_REQUEST_CONTEXT
        .try_with(|ctx| ctx.route.as_ref().to_string())
        .ok()
}

/// Execute the provided future with the supplied request context available to downstream spans.
pub async fn with_request_context<Fut, T>(
    request_id: impl Into<String>,
    route: impl Into<String>,
    fut: Fut,
) -> T
where
    Fut: Future<Output = T>,
{
    let context = RequestContext {
        request_id: Arc::from(request_id.into()),
        route: Arc::from(route.into()),
    };
    ACTIVE_REQUEST_CONTEXT.scope(context, fut).await
}

#[derive(Clone)]
struct RequestContext {
    request_id: Arc<str>,
    route: Arc<str>,
}

tokio::task_local! {
    static ACTIVE_REQUEST_CONTEXT: RequestContext;
}

/// Factory for the `x-request-id` generator layer.
#[must_use]
pub fn set_request_id_layer() -> SetRequestIdLayer<MakeRequestUuid> {
    SetRequestIdLayer::x_request_id(MakeRequestUuid)
}

/// Layer that propagates an incoming `x-request-id` header.
#[must_use]
pub fn propagate_request_id_layer() -> PropagateRequestIdLayer {
    PropagateRequestIdLayer::x_request_id()
}

/// Prometheus-backed metrics registry shared across services.
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    registry: Registry,
    http_requests_total: IntCounterVec,
    events_emitted_total: IntCounterVec,
    fsops_steps_total: IntCounterVec,
    active_torrents: IntGauge,
    queue_depth: IntGauge,
    engine_bytes_in: IntGauge,
    engine_bytes_out: IntGauge,
    config_watch_latency_ms: IntGauge,
    config_apply_latency_ms: IntGauge,
    config_update_failures_total: IntCounter,
    config_watch_slow_total: IntCounter,
    guardrail_violations_total: IntCounter,
    rate_limit_throttled_total: IntCounter,
}

/// Snapshot of selected gauges and counters for health reporting.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub active_torrents: i64,
    pub queue_depth: i64,
    pub config_watch_latency_ms: i64,
    pub config_apply_latency_ms: i64,
    pub config_update_failures_total: u64,
    pub config_watch_slow_total: u64,
    pub guardrail_violations_total: u64,
    pub rate_limit_throttled_total: u64,
}

impl Metrics {
    /// Construct a new metrics registry with the standard collectors registered.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the Prometheus collectors cannot be
    /// registered.
    pub fn new() -> Result<Self> {
        let registry = Registry::new();

        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests received"),
            &["route", "code"],
        )?;
        let events_emitted_total = IntCounterVec::new(
            Opts::new("events_emitted_total", "Domain events emitted by type"),
            &["type"],
        )?;
        let fsops_steps_total = IntCounterVec::new(
            Opts::new(
                "fsops_steps_total",
                "Filesystem post-processing steps executed by status",
            ),
            &["step", "status"],
        )?;
        let active_torrents =
            IntGauge::with_opts(Opts::new("active_torrents", "Number of active torrents"))?;
        let queue_depth =
            IntGauge::with_opts(Opts::new("queue_depth", "Queued torrent operations"))?;
        let engine_bytes_in =
            IntGauge::with_opts(Opts::new("engine_bytes_in", "Bytes received by the engine"))?;
        let engine_bytes_out =
            IntGauge::with_opts(Opts::new("engine_bytes_out", "Bytes sent by the engine"))?;
        let config_watch_latency_ms = IntGauge::with_opts(Opts::new(
            "config_watch_latency_ms",
            "Time spent waiting for configuration updates (ms)",
        ))?;
        let config_apply_latency_ms = IntGauge::with_opts(Opts::new(
            "config_apply_latency_ms",
            "Time taken to apply configuration updates (ms)",
        ))?;
        let config_update_failures_total = IntCounter::with_opts(Opts::new(
            "config_update_failures_total",
            "Configuration update failures",
        ))?;
        let config_watch_slow_total = IntCounter::with_opts(Opts::new(
            "config_watch_slow_total",
            "Configuration updates exceeding the latency guard rail",
        ))?;
        let guardrail_violations_total = IntCounter::with_opts(Opts::new(
            "config_guardrail_violations_total",
            "Configuration and setup guardrail violations",
        ))?;
        let rate_limit_throttled_total = IntCounter::with_opts(Opts::new(
            "api_rate_limit_throttled_total",
            "Requests rejected due to API rate limiting",
        ))?;

        registry.register(Box::new(http_requests_total.clone()))?;
        registry.register(Box::new(events_emitted_total.clone()))?;
        registry.register(Box::new(fsops_steps_total.clone()))?;
        registry.register(Box::new(active_torrents.clone()))?;
        registry.register(Box::new(queue_depth.clone()))?;
        registry.register(Box::new(engine_bytes_in.clone()))?;
        registry.register(Box::new(engine_bytes_out.clone()))?;
        registry.register(Box::new(config_watch_latency_ms.clone()))?;
        registry.register(Box::new(config_apply_latency_ms.clone()))?;
        registry.register(Box::new(config_update_failures_total.clone()))?;
        registry.register(Box::new(config_watch_slow_total.clone()))?;
        registry.register(Box::new(guardrail_violations_total.clone()))?;
        registry.register(Box::new(rate_limit_throttled_total.clone()))?;

        Ok(Self {
            inner: Arc::new(MetricsInner {
                registry,
                http_requests_total,
                events_emitted_total,
                fsops_steps_total,
                active_torrents,
                queue_depth,
                engine_bytes_in,
                engine_bytes_out,
                config_watch_latency_ms,
                config_apply_latency_ms,
                config_update_failures_total,
                config_watch_slow_total,
                guardrail_violations_total,
                rate_limit_throttled_total,
            }),
        })
    }

    /// Increment the HTTP request counter for the given route and status code.
    pub fn inc_http_request(&self, route: &str, status: u16) {
        self.inner
            .http_requests_total
            .with_label_values(&[route, &status.to_string()])
            .inc();
    }

    /// Increment the emitted event counter for the specific event type.
    pub fn inc_event(&self, event_type: &str) {
        self.inner
            .events_emitted_total
            .with_label_values(&[event_type])
            .inc();
    }

    /// Increment the filesystem post-processing step counter.
    pub fn inc_fsops_step(&self, step: &str, status: &str) {
        self.inner
            .fsops_steps_total
            .with_label_values(&[step, status])
            .inc();
    }

    /// Set the active torrent gauge.
    pub fn set_active_torrents(&self, count: i64) {
        self.inner.active_torrents.set(count);
    }

    /// Set the queue depth gauge.
    pub fn set_queue_depth(&self, depth: i64) {
        self.inner.queue_depth.set(depth);
    }

    /// Record inbound bytes observed by the engine.
    pub fn set_engine_bytes_in(&self, value: i64) {
        self.inner.engine_bytes_in.set(value);
    }

    /// Record outbound bytes observed by the engine.
    pub fn set_engine_bytes_out(&self, value: i64) {
        self.inner.engine_bytes_out.set(value);
    }

    /// Record the observed latency while waiting for configuration updates.
    pub fn observe_config_watch_latency(&self, duration: Duration) {
        self.inner
            .config_watch_latency_ms
            .set(Self::duration_to_ms(duration));
    }

    /// Record the observed latency for applying configuration updates.
    pub fn observe_config_apply_latency(&self, duration: Duration) {
        self.inner
            .config_apply_latency_ms
            .set(Self::duration_to_ms(duration));
    }

    /// Increment the configuration update failure counter.
    pub fn inc_config_update_failure(&self) {
        self.inner.config_update_failures_total.inc();
    }

    /// Increment the counter tracking slow configuration applications.
    pub fn inc_config_watch_slow(&self) {
        self.inner.config_watch_slow_total.inc();
    }

    /// Increment the guardrail violation counter (e.g. setup loopback enforcement).
    pub fn inc_guardrail_violation(&self) {
        self.inner.guardrail_violations_total.inc();
    }

    /// Increment the API rate limiter throttle counter.
    pub fn inc_rate_limit_throttled(&self) {
        self.inner.rate_limit_throttled_total.inc();
    }

    /// Render the metrics registry using the Prometheus text exposition format.
    ///
    /// # Errors
    ///
    /// Returns an error if the metrics cannot be encoded or if the encoded
    /// buffer is not valid UTF-8.
    pub fn render(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.inner.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .context("failed to encode Prometheus metrics")?;
        String::from_utf8(buffer).context("metrics output was not valid UTF-8")
    }

    /// Take a point-in-time snapshot of the most relevant gauges and counters.
    #[must_use]
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            active_torrents: self.inner.active_torrents.get(),
            queue_depth: self.inner.queue_depth.get(),
            config_watch_latency_ms: self.inner.config_watch_latency_ms.get(),
            config_apply_latency_ms: self.inner.config_apply_latency_ms.get(),
            config_update_failures_total: self.inner.config_update_failures_total.get(),
            config_watch_slow_total: self.inner.config_watch_slow_total.get(),
            guardrail_violations_total: self.inner.guardrail_violations_total.get(),
            rate_limit_throttled_total: self.inner.rate_limit_throttled_total.get(),
        }
    }

    fn duration_to_ms(duration: Duration) -> i64 {
        i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
    }
}

/// Persist an `OpenAPI` JSON document to disk and return the canonicalised payload.
///
/// # Errors
///
/// Returns an error if the output directory cannot be created, the document
/// cannot be serialised, or the file cannot be written.
pub fn persist_openapi(path: impl AsRef<Path>, document: &Value) -> Result<String> {
    let json = serde_json::to_string_pretty(document)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create OpenAPI output directory '{}'",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, json.as_bytes())
        .with_context(|| format!("failed to write OpenAPI artifact to '{}'", path.display()))?;
    Ok(json)
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
