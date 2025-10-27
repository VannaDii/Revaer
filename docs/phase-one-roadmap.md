# Phase One Roadmap

_Last updated: 2025-10-16_

This document captures the current delta between the Phase One objective and the existing codebase. It should be kept in sync as work progresses across the eight workstreams.

## Snapshot

| Workstream               | Current State                                                                               | Key Gaps                                                                                                                           | Immediate Actions                                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Control Plane & Setup    | Postgres schema, ConfigService watcher, setup CLI/API, immutable-key guard, history logging; loopback enforcement + RFC7807 pointers live | Engine hot-reload not yet exercising throttles; setup token lifecycle/error telemetry still thin                                     | Add watcher-driven throttle tests, expand setup diagnostics and rate-limit guardrails                                              |
| Torrent Domain & Adapter | Event bus, orchestrator scaffold, enriched torrent DTOs, stub session worker now persists resume metadata/fastresume, reconciles selection/sequential flags, enforces throttle guard rails, and surfaces degraded health | Real libtorrent FFI binding and alert pump still pending; need to exercise live fast-resume blobs and real libtorrent rate/health controls | Replace stub session with libtorrent bindings, translate real alerts, and validate against native libtorrent in the feature-gated suite |
| File Selection & FsOps   | Idempotent FsOps pipeline extracts zip payloads, flattens single directories, enforces allow lists, records `.revaer.meta`, and applies move/copy/hardlink transfers with chmod/chown/umask handling | Extraction currently limited to zip archives, PAR2 stage still absent, cleanup rules lack checksum awareness, pipeline assumes Unix tooling for ownership changes | Expand extractor matrix (7z/tar), add PAR2 verification, surface non-Unix fallbacks, and extend cleanup/telemetry coverage |
| Public HTTP API & SSE    | Admin setup/settings/torrent CRUD, SSE stream, metrics stub, OpenAPI generator, initial `/api/v2/*` qB façade (auth stub, version, torrents info/add/pause/resume/delete, transfer limits) | `/v1/torrents/*` pagination/filter matrix still partial, qB façade lacks differential sync, cookie-auth hardening, and advanced operations (rename, categories); SSE replay still missing Last-Event-ID coverage       | Finish pagination/filter story, tighten façade auth/session handling, add incremental sync endpoints, and expand SSE regression tests |
| CLI Parity               | Supports setup start/complete, settings patch, admin torrent add/remove (magnet-aware), status | Missing `select`, `action`, `ls`, `status` detail view, `tail` SSE client, richer validation                                        | Extend CLI command surface to mirror API, add reconnecting SSE tailer, flesh out filtering and exit-code contract                 |
| Security & Observability | API key storage hashed, tracing initialised, metrics registry struct                        | No per-key rate limits, no X-RateLimit headers, magnet/body bounds missing, tracing not propagated to engine/fsops, metrics unused | Introduce token-bucket middleware, enforce payload bounds, propagate spans through orchestrator/fsops, export Prometheus counters |
| CI & Packaging           | Workspace compiles, justfile for fmt/lint/test                                              | No CI workflows, cargo-deny/audit missing, no env access guard, no Docker packaging or healthcheck                                 | Author GitHub Actions (lint, security, tests, build), enforce env guard lint, build minimal non-root container with HEALTHCHECK   |
| Operational End-to-End   | Bootstrap skeleton and event bus exist                                                      | Torrent download, fs pipeline, restart resume, throttling, degraded health scenarios unimplemented                                 | Sequence implementation/testing to satisfy runbook once engine/fsops/API parity are in place                                      |

## Remaining Scope Specification

### 1. Torrent Engine Integration

- Swap the stubbed `LibtSession` for the real libtorrent binding so the existing worker drives a native session while continuing to process commands for add/pause/resume/remove, sequential toggles, rate limits, selection updates, reannounce, and force-recheck.
- Validate persisted fast-resume payloads, priorities, target directories, and sequential flags against the live session on startup; continue emitting reconciliation events when divergence is detected.
- Translate libtorrent alerts into EventBus messages (`FilesDiscovered`, `Progress`, `StateChanged`, `Completed`, `Failure`) while respecting the ≤10 Hz per-torrent coalescing rule; recover from alert polling failures by degrading health and attempting bounded restarts.
- Ensure global and per-torrent rate caps driven by `engine_profile` updates are enforced by libtorrent within two seconds, with audit logs surfaced when caps change.
- Extend the feature-gated integration suite to execute against the native libtorrent build (resume restore, rate-limit enforcement, alert mapping) in addition to the in-process stub.

### 2. File Selection & FsOps Pipeline

- Keep include/exclude glob logic aligned with torrent selection so priority updates continue to reflect operator intent, including the `@skip_fluff` preset.
- Extend the FsOps pipeline to additional archive formats (7z/tar), introduce the PAR2 verification/repair stage, and surface checksum metadata alongside the recorded `.revaer.meta` entries.
- Add non-Unix fallbacks or clear operator guidance when ownership/umask directives cannot be honoured, and surface the condition via events and `/health/full`.
- Harden dependency detection so missing extractor binaries trigger guarded degradation with actionable telemetry, then clear automatically once remediation succeeds.
- Broaden integration coverage to include error paths (permission denied, unsupported archive) and restart scenarios that reuse persisted metadata, capturing metrics snapshots for each stage.

### 3. Public HTTP API & SSE

- Round out `/v1/torrents` with cursor pagination, rich filtering (state, tracker, extension), and stabilise reannounce/recheck/sequential toggles with regression tests.
- Keep Problem+JSON responses consistent (including JSON Pointer metadata) and mirror them in CLI/user-facing tooling.
- Enhance SSE with Last-Event-ID replay, duplicate suppression, and resiliency tests covering torrent + FsOps event fan-out.
- Evolve the qB façade: honour incremental sync via `rid`, tighten the cookie/session model, surface categories/tags, and expose rename/reannounce operations.
- Expand health reporting to `/health/full`, document façade coverage in OpenAPI/mdBook, and add integration tests that exercise pagination, SSE replay, and façade flows end-to-end.

### 4. CLI Parity

- Add commands `revaer ls`, `status`, `select`, `action`, and `tail`, mirroring API filters, selection arguments (include/exclude/skip-fluff), sequential toggles, and data deletion flags.
- Implement an SSE tailer that reconnects on failure, honors Last-Event-ID, and avoids duplicate terminal output.
- Standardize exit codes (0 success, 2 validation, >2 runtime failures) and surface RFC7807 payloads, including pointer metadata, in human-readable CLI output.
- Provide CLI integration tests that run against the API fixture stack, covering filter combinations, sequential toggles, and tail reconnection behaviour.

### 5. Security & Observability

- Introduce API key lifecycle endpoints (issue, rotate, revoke) with hashed-at-rest storage, returning secrets only once; enforce per-key token-bucket rate limiting and include `X-RateLimit-*` headers.
- Harden inputs by bounding magnet length, multipart size, filter glob counts, and header values; return Problem+JSON validation errors without panics for malformed requests.
- Propagate tracing spans (request IDs) through the API, engine, and FsOps layers; ensure metrics cover HTTP status, event flow, queue depth, libtorrent transfer, and FsOps step durations, exposed via `/metrics`.
- Reflect degraded health when tools are missing, engine sessions fault, or queue depth exceeds thresholds; emit corresponding `SettingsChanged` and `HealthChanged` events.
- Document operational expectations for rate limiting, key rotation, and observability dashboards.

### 6. CI & Packaging

- Create GitHub Actions (or equivalent) workflows for formatting (`cargo fmt`), linting (`cargo clippy -D warnings`), security scans (`cargo deny`, `cargo audit`), tests (unit/integration with Postgres and libtorrent behind an opt-in guard), and cross-compilation artifacts for Linux x86_64 and aarch64.
- Enforce an environment-access lint that fails CI if `std::env` reads occur outside the composition root (excluding `DATABASE_URL`).
- Produce a non-root Docker image with read-only root filesystem, declared volumes, and a healthcheck hitting `/health`; ensure runtime documentation validates within the image.
- Publish build artifacts and container digests with provenance metadata; wire CI status into the roadmap release checklist.

### 7. Operational Runbook Automation

- Author a script to execute the full phase objective on both x86_64 and aarch64: bootstrap using `DATABASE_URL`, complete setup token flow, add a magnet, monitor `FilesDiscovered`/`Progress`/`Completed`, run FsOps, simulate crash/restart with fast-resume recovery, adjust throttles, and validate degraded health when extractors are absent.
- Capture assertions and logs for each phase, producing artifacts suitable for runbook review and CI retention; ensure failures mark the engine or pipeline health accordingly.
- Include cleanup routines to return environments to a reusable state while retaining diagnostic logs.

### 8. Documentation & Final Polish

- Update `docs/phase-one-roadmap.md` continuously and add ADRs covering engine architecture, FsOps design, API/CLI contracts, and security posture.
- Regenerate `docs/api/openapi.json` alongside illustrative request/response examples for new endpoints.
- Extend user-facing guides for CLI usage, health/metrics references, and operational setup covering API keys, rate limits, and degraded-mode recovery.
- Provide a final Phase One release checklist that ties documentation, runbook, and CI artifacts together.

## Next Steps Tracking

1. Land setup/network hardening and control-plane polish.
2. Replace the stub worker with a real libtorrent session, resume store, and alert-driven event bridge.
3. Implement FsOps pipeline with allow-listed execution and metadata.
4. Expose `/v1/*` APIs + CLI parity and reinforce security/observability.
5. Stand up CI, packaging, and full runbook validation.
