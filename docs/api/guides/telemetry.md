# Telemetry & Metrics Reference

Revaer exposes structured telemetry via Prometheus metrics, tracing spans, CLI events, and health endpoints.

## Prometheus Metrics

Scrape `/metrics` to collect the following key series:

- `http_requests_total{route,code}`: HTTP request counts per route/status.
- `events_emitted_total{type}`: EventBus emissions by domain event type.
- `fsops_steps_total{step,status}`: Counts of FsOps pipeline steps and their outcomes.
- `api_rate_limit_throttled_total`: Requests rejected due to API rate limiting.
- `config_guardrail_violations_total`: Guard-rail incidents (loopback guard, rate limit misconfig, watcher failures).
- `active_torrents` / `queue_depth`: Current torrent load snapshot.
- `config_watch_latency_ms` / `config_apply_latency_ms`: Latest watcher latencies.

## Health Endpoints

- `GET /health`: Lightweight readiness probe (database reachability).
- `GET /health/full`: Extended status including build sha, degraded components, and the metric snapshot above.
- Guard-rail violations (e.g., missing rate limits, FsOps failure) populate the `degraded` array and emit `HealthChanged` events.

## Tracing

- The API attaches `x-request-id` to every response and propagates it through `tracing` spans.
- The CLI generates a new UUID per invocation and sends it via `x-request-id`, enabling correlation between CLI telemetry events, API logs, and downstream FsOps traces.

## CLI Telemetry

- When `REVAER_TELEMETRY_ENDPOINT` is configured, each command emits a JSON event (`command`, `outcome`, `trace_id`, `exit_code`, optional `message`, `timestamp_ms`).
- Use these events to build operator dashboards (success/error rates, latency histograms) without scraping CLI stdout.

## Runbook Validation

- The runbook (`docs/runbook.md`) captures expected metric deltas (FsOps counters, rate-limit throttles) and health transitions, serving as acceptance evidence for new releases.
