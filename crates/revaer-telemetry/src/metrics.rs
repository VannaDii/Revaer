//! Prometheus-backed metrics registry and snapshot helpers.
//!
//! # Design
//! - Encapsulates collector registration to keep the public API small.
//! - Exposes a minimal set of counters/gauges relevant to Revaer services.

use std::convert::TryFrom;
use std::time::Duration;

use crate::error::{Result, TelemetryError};
use prometheus::core::Collector;
use prometheus::{Encoder, IntCounter, IntCounterVec, IntGauge, Opts, Registry, TextEncoder};
use serde::Serialize;

/// Prometheus-backed metrics registry shared across services.
#[derive(Clone)]
pub struct Metrics {
    inner: std::sync::Arc<MetricsInner>,
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

struct MetricsCollectors {
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
    /// Current number of active torrents.
    pub active_torrents: i64,
    /// Current queue depth for pending torrents.
    pub queue_depth: i64,
    /// Latest latency (ms) when watching for configuration changes.
    pub config_watch_latency_ms: i64,
    /// Latest latency (ms) when applying configuration changes.
    pub config_apply_latency_ms: i64,
    /// Total count of configuration update failures observed.
    pub config_update_failures_total: u64,
    /// Total count of slow configuration watch intervals observed.
    pub config_watch_slow_total: u64,
    /// Total guardrail violations recorded.
    pub guardrail_violations_total: u64,
    /// Total requests throttled by API rate limiting.
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
        let inner = MetricsInner::new(Registry::new())?;
        Ok(Self {
            inner: std::sync::Arc::new(inner),
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
            .map_err(|source| TelemetryError::MetricsEncode { source })?;
        String::from_utf8(buffer).map_err(|source| TelemetryError::MetricsUtf8 { source })
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

    /// Convert a duration to milliseconds saturating at `i64::MAX`.
    pub(crate) fn duration_to_ms(duration: Duration) -> i64 {
        i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
    }
}

impl MetricsInner {
    fn new(registry: Registry) -> Result<Self> {
        let collectors = MetricsCollectors::new()?;
        collectors.register_all(&registry)?;
        let MetricsCollectors {
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
        } = collectors;

        Ok(Self {
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
        })
    }
}

impl MetricsCollectors {
    fn new() -> Result<Self> {
        Ok(Self {
            http_requests_total: counter_vec(
                "http_requests_total",
                "Total HTTP requests received",
                &["route", "code"],
            )?,
            events_emitted_total: counter_vec(
                "events_emitted_total",
                "Domain events emitted by type",
                &["type"],
            )?,
            fsops_steps_total: counter_vec(
                "fsops_steps_total",
                "Filesystem post-processing steps executed by status",
                &["step", "status"],
            )?,
            active_torrents: gauge("active_torrents", "Number of active torrents")?,
            queue_depth: gauge("queue_depth", "Queued torrent operations")?,
            engine_bytes_in: gauge("engine_bytes_in", "Bytes received by the engine")?,
            engine_bytes_out: gauge("engine_bytes_out", "Bytes sent by the engine")?,
            config_watch_latency_ms: gauge(
                "config_watch_latency_ms",
                "Time spent waiting for configuration updates (ms)",
            )?,
            config_apply_latency_ms: gauge(
                "config_apply_latency_ms",
                "Time taken to apply configuration updates (ms)",
            )?,
            config_update_failures_total: counter(
                "config_update_failures_total",
                "Configuration update failures",
            )?,
            config_watch_slow_total: counter(
                "config_watch_slow_total",
                "Configuration updates exceeding the latency guard rail",
            )?,
            guardrail_violations_total: counter(
                "config_guardrail_violations_total",
                "Configuration and setup guardrail violations",
            )?,
            rate_limit_throttled_total: counter(
                "api_rate_limit_throttled_total",
                "Requests rejected due to API rate limiting",
            )?,
        })
    }

    fn register_all(&self, registry: &Registry) -> Result<()> {
        register_collector(
            registry,
            "http_requests_total",
            self.http_requests_total.clone(),
        )?;
        register_collector(
            registry,
            "events_emitted_total",
            self.events_emitted_total.clone(),
        )?;
        register_collector(
            registry,
            "fsops_steps_total",
            self.fsops_steps_total.clone(),
        )?;
        register_collector(registry, "active_torrents", self.active_torrents.clone())?;
        register_collector(registry, "queue_depth", self.queue_depth.clone())?;
        register_collector(registry, "engine_bytes_in", self.engine_bytes_in.clone())?;
        register_collector(registry, "engine_bytes_out", self.engine_bytes_out.clone())?;
        register_collector(
            registry,
            "config_watch_latency_ms",
            self.config_watch_latency_ms.clone(),
        )?;
        register_collector(
            registry,
            "config_apply_latency_ms",
            self.config_apply_latency_ms.clone(),
        )?;
        register_collector(
            registry,
            "config_update_failures_total",
            self.config_update_failures_total.clone(),
        )?;
        register_collector(
            registry,
            "config_watch_slow_total",
            self.config_watch_slow_total.clone(),
        )?;
        register_collector(
            registry,
            "config_guardrail_violations_total",
            self.guardrail_violations_total.clone(),
        )?;
        register_collector(
            registry,
            "api_rate_limit_throttled_total",
            self.rate_limit_throttled_total.clone(),
        )?;
        Ok(())
    }
}

fn counter_vec(name: &'static str, help: &'static str, labels: &[&str]) -> Result<IntCounterVec> {
    IntCounterVec::new(Opts::new(name, help), labels)
        .map_err(|source| TelemetryError::MetricsCollector { name, source })
}

fn counter(name: &'static str, help: &'static str) -> Result<IntCounter> {
    IntCounter::with_opts(Opts::new(name, help))
        .map_err(|source| TelemetryError::MetricsCollector { name, source })
}

fn gauge(name: &'static str, help: &'static str) -> Result<IntGauge> {
    IntGauge::with_opts(Opts::new(name, help))
        .map_err(|source| TelemetryError::MetricsCollector { name, source })
}

fn register_collector<C>(registry: &Registry, name: &'static str, collector: C) -> Result<()>
where
    C: Collector + Clone + 'static,
{
    registry
        .register(Box::new(collector))
        .map_err(|source| TelemetryError::MetricsRegister { name, source })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn duration_to_ms_saturates_on_large_values() {
        let duration = Duration::from_secs(u64::MAX / 2);
        assert_eq!(Metrics::duration_to_ms(duration), i64::MAX);
    }

    #[test]
    fn metrics_snapshot_reflects_updates() -> Result<()> {
        let metrics = Metrics::new()?;
        metrics.inc_http_request("/health", 200);
        metrics.inc_event("torrent_added");
        metrics.inc_fsops_step("transfer", "completed");
        metrics.set_active_torrents(5);
        metrics.set_queue_depth(2);
        metrics.set_engine_bytes_in(1_024);
        metrics.set_engine_bytes_out(2_048);
        metrics.observe_config_watch_latency(Duration::from_millis(120));
        metrics.observe_config_apply_latency(Duration::from_millis(45));
        metrics.inc_config_update_failure();
        metrics.inc_config_watch_slow();
        metrics.inc_guardrail_violation();
        metrics.inc_rate_limit_throttled();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active_torrents, 5);
        assert_eq!(snapshot.queue_depth, 2);
        assert_eq!(snapshot.config_watch_latency_ms, 120);
        assert_eq!(snapshot.config_apply_latency_ms, 45);
        assert_eq!(snapshot.config_update_failures_total, 1);
        assert_eq!(snapshot.config_watch_slow_total, 1);
        assert_eq!(snapshot.guardrail_violations_total, 1);
        assert_eq!(snapshot.rate_limit_throttled_total, 1);

        let rendered = metrics.render()?;
        assert!(rendered.contains("http_requests_total"));
        assert!(rendered.contains("fsops_steps_total"));
        assert!(rendered.contains("config_guardrail_violations_total"));
        Ok(())
    }
}
