# 004 – Phase One Delivery Track

- Status: Accepted
- Date: 2025-10-17

## Motivation

Phase One bundles the remaining work required to transition Revaer from the current stubs into a production-ready torrent orchestration platform. This record captures the implementation notes, decisions, and verification evidence for each workstream item enumerated in `docs/phase-one-roadmap.md`.

## Design Notes

- Follow the library-first structure outlined in `AGENT.md` with crate-specific modules for configuration, engine integration, filesystem operations, public API, CLI, security, and packaging.
- Apply tight configuration validation and hot-reload behaviour to guarantee that throttle and policy updates propagate within two seconds.
- Emit guard-rail telemetry whenever global throttles are disabled, driven to zero, or configured above the 5 Gbps warning threshold so operators can react quickly.
- Replace the stub libtorrent adapter with a session worker that owns state, persists fast-resume metadata, and surfaces alert-driven events with bounded fan-out.
- Persist resume metadata and fastresume payloads via `FastResumeStore`, reconcile on startup, and emit `SelectionReconciled` events plus health degradations when store contents diverge or writes fail.
- Build deterministic include/exclude rule evaluation and an idempotent FsOps pipeline anchored by `.revaer.meta`.
- Expose a consistent Problem+JSON contract across HTTP and CLI surfaces, including pagination and SSE replay support.
- Enforce observability invariants: structured tracing with context propagation, bounded rate limits, Prometheus metrics, and degraded health signalling when dependencies fail.
- Ensure every workflow is reproducible via `just` targets and validated in CI, with container packaging aligned to the non-root, read-only expectations.
- Follow the canonical `just` recipe surface (fmt, lint, test, ci, etc.). Coloned variants are mapped to hyphenated recipe names (`fmt-fix`, `build-release`, `api-export`) because `just` 1.43.0 rejects colons in recipe identifiers without unstable modules; the semantics remain identical.

## Test Coverage Summary

- `just ci` serves as the baseline verification target. Each workstream delivers focused unit tests, integration coverage, and feature-flagged live tests (for libtorrent, Postgres, FsOps).
- Coverage gates are enforced via `cargo llvm-cov` with `--fail-under 80` across library crates.
- Integration suites will rely on `testcontainers` (Postgres, libtorrent) and workspace-specific fixtures for FsOps pipelines and API/CLI flows, including the configuration watcher hot-reload test and new libtorrent-feature tests for resume restoration and fastresume persistence.

## Outcome

- All public surfaces now enforce API-key authentication with token-bucket rate limiting, `429` Problem+JSON responses, and telemetry counters exported via Prometheus and `/health/full`.
- SSE endpoints honour the same auth and Last-Event-ID semantics, with CLI resume support persisting state between reconnects.
- The CLI propagates `x-request-id`, standardises exit codes (`0` success, `2` validation, `3` runtime), and emits optional telemetry events to `REVAER_TELEMETRY_ENDPOINT`.
- A release-ready Docker image (`Dockerfile`) packages the API binary and documentation on a non-root, read-only-friendly runtime with health checks and volume mounts for config/data.
- CI now publishes release artefacts (`revaer-app`, OpenAPI) and runs MSRV and container security jobs via `just` targets; binaries are checksummed alongside provenance metadata.
- Documentation additions cover FsOps design, API/CLI contracts, security posture, operator runbook, telemetry reference, and the phase-one release checklist.

## Observability Updates

- Telemetry enhancements include structured logs for setup token issuance/consumption, loopback enforcement failures, configuration watcher updates, rate-limit guard-rail decisions, and resume store degradation/recovery.
- Metrics will expand to track HTTP request outcomes, SSE fan-out, event queue depth, torrent throughput, FsOps step durations, and health degradation counts.
- `/health/full` will report engine, FsOps, and database readiness with latency measurements and revision hashes, mirrored by CLI status commands.

## Risk & Rollback Plan

- Maintain incremental commits gated by `just ci` to isolate regressions. Any new dependency introductions require explicit justification and fallbacks documented here.
- Where feature flags guard libtorrent integration, provide mockable interfaces so tests can fall back to stub implementations if the environment lacks native bindings.
- Persist fast-resume metadata and `.revaer.meta` files so failed deployments can roll back without corrupting state; ensure migrations remain additive.

## Dependency Rationale

No new dependencies have been added yet. Future additions (e.g., libtorrent bindings, glob evaluators, archive tools) must include:
- Why the crate/tool is necessary.
- Alternatives considered (including bespoke implementations) and why they were rejected.
- Security and maintenance assessment (license compatibility, release cadence).
