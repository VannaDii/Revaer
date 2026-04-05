# Phase One Roadmap

_Last updated: 2026-04-04_

This document captures the current delta between the Phase One objective and the existing codebase. It should be kept in sync as work progresses across the eight workstreams.

## Snapshot

| Workstream               | Current State                                                                                                                                                                                                                      | Key Gaps                                                                                                                                                                                                 | Immediate Actions                                                                                                                                                                      |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Control Plane & Setup    | Postgres schema, ConfigService watcher, setup CLI/API, immutable-key guard, history logging; loopback enforcement + RFC7807 pointers live                                                                                          | Engine hot-reload not yet exercising throttles; setup token lifecycle/error telemetry still thin                                                                                                         | Add watcher-driven throttle tests, expand setup diagnostics and rate-limit guardrails                                                                                                  |
| Torrent Domain & Adapter | Native libtorrent FFI (cxx) restored and default-enabled; session worker with alert pump/resume store, throttles, selection, and degraded health surfaced via event bus; stub path retained only when the feature is disabled      | Native CI coverage exists, but alert/rate-limit regression coverage is still thin and broader validation of resume reconciliation and failure handling is still needed                                  | Deepen alert/rate-limit/resume validation and harden failure handling                                                                                                                   |
| File Selection & FsOps   | Idempotent FsOps pipeline now extracts `zip`/`tar`/`tar.gz`/`tgz` archives in-process, supports guarded `7z`/`rar` extraction via external tools, runs PAR2 verify/repair stages, records checksum metadata in `.revaer.meta`, and applies move/copy/hardlink transfers with chmod/chown/umask handling | `7z`/`rar` and PAR2 still depend on host tooling being installed; ownership overrides remain Unix-only by design, and broader recovery/failure coverage should keep expanding                            | Keep hardening extractor/PAR2 recovery scenarios, document host-tool prerequisites clearly, and expand FsOps telemetry + restart-path coverage                                         |
| Public HTTP API & SSE    | Admin setup/settings/torrent CRUD, SSE stream, metrics endpoint, OpenAPI generator, `/api/v2/*` qB façade with cookie sessions, rename/category/tag mutation, relocate, reannounce/recheck, transfer limits, and incremental `rid` sync | `/v1/torrents/*` pagination/filter matrix still partial; qB coverage is intentionally bounded rather than full parity; SSE replay still needs broader Last-Event-ID regression coverage                 | Finish pagination/filter story, document deliberate qB compatibility scope, and expand SSE replay regression tests                                                                      |
| CLI Parity               | Supports setup start/complete, settings patch, torrent add/remove/list/status/select/action flows, and CLI wrappers around config + torrent APIs                                                                                   | SSE tail UX and richer validation/diagnostic coverage still need hardening                                                                                                                                 | Expand reconnecting tail coverage and tighten validation/exit-code contracts                                                                                                            |
| Security & Observability | API key storage hashed, per-key rate limits and `X-RateLimit-*` headers exposed, tracing initialized, metrics registry exported, and dashboard metrics now sourced from runtime state                                              | OTEL exporter path was placeholder-only and now needs operational validation; tracing/metrics coverage should keep expanding across engine/fsops failure paths                                             | Validate OTLP exporter behavior in deployment flows and keep expanding engine/fsops observability coverage                                                                              |
| CI & Packaging           | GitHub Actions cover fmt/lint/deny/audit/tests/cov via `just ci`; native libtorrent CI exists; Dockerfile builds non-root image with bundled libtorrent and HEALTHCHECK; docs workflow publishes mdBook; image workflow now scans, attests, and signs published images | Rootfs posture remains documented rather than enforced, and image hardening still needs broader cross-arch/runtime validation                                                                             | Keep image provenance/scan/sign gates in CI, harden container runtime guidance, and extend cross-arch/runtime validation                                                               |
| Operational End-to-End   | Playwright-backed API/UI flows run via `just ui-e2e`, and `just runbook` now packages repeatable validation artifacts                                                                                                              | Manual fault-injection drills still exist for extractor/permission/recovery scenarios                                                                                                                      | Keep automating the remaining runbook drills while retaining the operator-facing checklist                                                                                              |

## Remaining Scope Specification

### 1. Torrent Engine Integration

-   Harden the native libtorrent session: keep the stub only for feature-off builds while ensuring the default path drives the real adapter for add/pause/resume/remove, sequential toggles, rate limits, selection updates, reannounce, and force-recheck.
-   Validate persisted fast-resume payloads, priorities, target directories, and sequential flags against the live session on startup; continue emitting reconciliation events when divergence is detected.
-   Translate libtorrent alerts into EventBus messages (`FilesDiscovered`, `Progress`, `StateChanged`, `Completed`, `Failure`) while respecting the ≤10 Hz per-torrent coalescing rule; recover from alert polling failures by degrading health and attempting bounded restarts.
-   Ensure global and per-torrent rate caps driven by `engine_profile` updates are enforced by libtorrent within two seconds, with audit logs surfaced when caps change.
-   Extend the feature-gated integration suite to execute against the native libtorrent build (resume restore, rate-limit enforcement, alert mapping) in addition to the in-process stub.

### 2. File Selection & FsOps Pipeline

-   Keep include/exclude glob logic aligned with torrent selection so priority updates continue to reflect operator intent, including the `@skip_fluff` preset.
-   Extend the FsOps pipeline to additional archive formats (7z/tar), introduce the PAR2 verification/repair stage, and surface checksum metadata alongside the recorded `.revaer.meta` entries.
-   Add non-Unix fallbacks or clear operator guidance when ownership/umask directives cannot be honoured, and surface the condition via events and `/health/full`.
-   Harden dependency detection so missing extractor binaries trigger guarded degradation with actionable telemetry, then clear automatically once remediation succeeds.
-   Broaden integration coverage to include error paths (permission denied, unsupported archive) and restart scenarios that reuse persisted metadata, capturing metrics snapshots for each stage.

### 3. Public HTTP API & SSE

-   Round out `/v1/torrents` with cursor pagination, rich filtering (state, tracker, extension), and stabilise reannounce/recheck/sequential toggles with regression tests.
-   Keep Problem+JSON responses consistent (including JSON Pointer metadata) and mirror them in CLI/user-facing tooling.
-   Enhance SSE with Last-Event-ID replay, duplicate suppression, and resiliency tests covering torrent + FsOps event fan-out.
-   Evolve the qB façade: tighten the cookie/session model, surface removals/categories/tags in incremental sync, and expose rename/reannounce operations.
-   Expand health reporting to `/health/full`, document façade coverage in OpenAPI/mdBook, and add integration tests that exercise pagination, SSE replay, and façade flows end-to-end.

### 4. CLI Parity

-   Add commands `revaer ls`, `status`, `select`, `action`, and `tail`, mirroring API filters, selection arguments (include/exclude/skip-fluff), sequential toggles, and data deletion flags.
-   Implement an SSE tailer that reconnects on failure, honors Last-Event-ID, and avoids duplicate terminal output.
-   Standardize exit codes (0 success, 2 validation, >2 runtime failures) and surface RFC7807 payloads, including pointer metadata, in human-readable CLI output.
-   Provide CLI integration tests that run against the API fixture stack, covering filter combinations, sequential toggles, and tail reconnection behaviour.

### 5. Security & Observability

-   Introduce API key lifecycle endpoints (issue, rotate, revoke) with hashed-at-rest storage, returning secrets only once; enforce per-key token-bucket rate limiting and include `X-RateLimit-*` headers.
-   Harden inputs by bounding magnet length, multipart size, filter glob counts, and header values; return Problem+JSON validation errors without panics for malformed requests.
-   Propagate tracing spans (request IDs) through the API, engine, and FsOps layers; ensure metrics cover HTTP status, event flow, queue depth, libtorrent transfer, and FsOps step durations, exposed via `/metrics`.
-   Reflect degraded health when tools are missing, engine sessions fault, or queue depth exceeds thresholds; emit corresponding `SettingsChanged` and `HealthChanged` events.
-   Document operational expectations for rate limiting, key rotation, and observability dashboards.

### 6. CI & Packaging

-   Keep GitHub Actions green across fmt/lint/deny/audit/tests/cov and add a matrix leg that runs the native libtorrent suite (REVAER_NATIVE_IT=1 with Docker host wiring).
-   Enforce an environment-access lint that fails CI if `std::env` reads occur outside the composition root (excluding `DATABASE_URL`).
-   Harden the container: retain non-root user, switch to read-only rootfs with explicit writable mounts, and gate builds with image scans and provenance/signing.
-   Produce cross-arch artifacts (x86_64/aarch64) and publish digests alongside build outputs and release notes.

### 7. Operational Runbook Automation

-   Author a script to execute the full phase objective on both x86_64 and aarch64: bootstrap using `DATABASE_URL`, complete setup token flow, add a magnet, monitor `FilesDiscovered`/`Progress`/`Completed`, run FsOps, simulate crash/restart with fast-resume recovery, adjust throttles, and validate degraded health when extractors are absent.
-   Capture assertions and logs for each phase, producing artifacts suitable for runbook review and CI retention; ensure failures mark the engine or pipeline health accordingly.
-   Include cleanup routines to return environments to a reusable state while retaining diagnostic logs.

### 8. Documentation & Final Polish

-   Update `docs/phase-one-roadmap.md` continuously and add ADRs covering engine architecture, FsOps design, API/CLI contracts, and security posture.
-   Regenerate `docs/api/openapi.json` alongside illustrative request/response examples for new endpoints.
-   Extend user-facing guides for CLI usage, health/metrics references, and operational setup covering API keys, rate limits, and degraded-mode recovery.
-   Provide a final Phase One release checklist that ties documentation, runbook, and CI artifacts together.

## Next Steps Tracking

1. Land setup/network hardening and control-plane polish.
2. Keep the native libtorrent session as the default, expand coverage (native CI leg, alert/rate-limit/resume validation), and preserve the stub only for feature-off builds.
3. Implement FsOps pipeline with allow-listed execution and metadata.
4. Expose `/v1/*` APIs + CLI parity and reinforce security/observability.
5. Stand up CI, packaging, and full runbook validation.
