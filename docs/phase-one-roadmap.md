# Phase One Roadmap

_Last updated: 2025-10-16_

This document captures the current delta between the Phase One objective and the existing codebase. It should be kept in sync as work progresses across the eight workstreams.

## Snapshot

| Workstream               | Current State                                                                               | Key Gaps                                                                                                                           | Immediate Actions                                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Control Plane & Setup    | Postgres schema, ConfigService watcher, setup CLI/API, immutable-key guard, history logging; loopback enforcement + RFC7807 pointers live | Engine hot-reload not yet exercising throttles; setup token lifecycle/error telemetry still thin                                     | Add watcher-driven throttle tests, expand setup diagnostics and rate-limit guardrails                                              |
| Torrent Domain & Adapter | Event bus, orchestrator scaffold, enriched torrent DTOs, libtorrent stub with command worker and state tracking | Real libtorrent session, disk-backed resume store, on-disk selection persistence, fast-resume optimisation still pending | Integrate libtorrent bindings under single task, persist resume/selection state, handle alert-driven event coalescing            |
| File Selection & FsOps   | FsOpsService emits lifecycle events and validates library root                              | No extraction/flatten/move-perms/cleanup pipeline, no `.revaer.meta`, no allow-list enforcement                                    | Model FsOps plan, implement idempotent steps with allow-list + metadata tracking, add fixtures/tests                              |
| Public HTTP API & SSE    | Admin setup/settings/torrent CRUD, SSE stream, metrics stub, OpenAPI generator              | `/v1/torrents/*` family absent, no cursor pagination/filters, SSE replay lacks Last-Event-ID tests, health endpoints minimal       | Define domain DTOs, implement public routes + filtering, extend SSE replay handling/tests, flesh out health                       |
| CLI Parity               | Supports setup start/complete, settings patch, admin torrent add/remove (magnet-aware), status | Missing `select`, `action`, `ls`, `status` detail view, `tail` SSE client, richer validation                                        | Extend CLI command surface to mirror API, add reconnecting SSE tailer, flesh out filtering and exit-code contract                 |
| Security & Observability | API key storage hashed, tracing initialised, metrics registry struct                        | No per-key rate limits, no X-RateLimit headers, magnet/body bounds missing, tracing not propagated to engine/fsops, metrics unused | Introduce token-bucket middleware, enforce payload bounds, propagate spans through orchestrator/fsops, export Prometheus counters |
| CI & Packaging           | Workspace compiles, justfile for fmt/lint/test                                              | No CI workflows, cargo-deny/audit missing, no env access guard, no Docker packaging or healthcheck                                 | Author GitHub Actions (lint, security, tests, build), enforce env guard lint, build minimal non-root container with HEALTHCHECK   |
| Operational End-to-End   | Bootstrap skeleton and event bus exist                                                      | Torrent download, fs pipeline, restart resume, throttling, degraded health scenarios unimplemented                                 | Sequence implementation/testing to satisfy runbook once engine/fsops/API parity are in place                                      |

## Next Steps Tracking

1. Land setup/network hardening and control-plane polish.
2. Replace the stub worker with a real libtorrent session, resume store, and alert-driven event bridge.
3. Implement FsOps pipeline with allow-listed execution and metadata.
4. Expose `/v1/*` APIs + CLI parity and reinforce security/observability.
5. Stand up CI, packaging, and full runbook validation.
