# 013 – Runtime Persistence for Torrents and FsOps Jobs

- Status: Accepted
- Date: 2025-10-27

## Motivation

- Phase One spec calls for a Postgres-backed runtime catalog to survive process restarts and surface torrent/Filesystem states to the API and CLI.
- Prior implementation only tracked runtime state in memory, so restarts lost visibility and FsOps progress could not be audited.
- Aligning with the spec removes the last major gap highlighted in the Phase One roadmap and unlocks future automation (retry queues, analytics).

## Design Notes

- Introduced a dedicated `revaer-runtime` crate that owns runtime migrations and a `RuntimeStore` facade wired through `sqlx`.
- Schema mirrors the spec (`revaer_runtime.torrents` + `fs_jobs`) with typed enums, timestamps, JSON file snapshots, and trigger-managed `updated_at`.
- `TorrentOrchestrator` now hydrates its catalog from the store on boot and persists every event (upsert/remove) to keep the DB authoritative.
- `FsOpsService` gained runtime hooks that record job starts, completions, and failures (including transfer mode & destination) alongside the existing `.revaer.meta`.
- Added integration tests (testcontainers Postgres) covering torrent upsert/remove and FsOps job transitions to guard the persistence layer.

## Test Coverage Summary

- New `crates/revaer-runtime/tests/runtime.rs` exercises the store end-to-end against real Postgres.
- Existing orchestrator/FsOps suites continue to cover event flow; runtime wiring is exercised indirectly via spawned tasks.
- `just ci` continues to be the required verification bundle (fmt, lint, udeps, audit, deny, test, cov).

## Observability Updates

- Runtime store persistence errors surface through `warn!` logs on the orchestrator/FsOps paths so operators can detect degraded durability.
- FsOps health events remain unchanged; job persistence mirrors those transitions for runbook inspection.

## Risk & Rollback

- Runtime persistence is additive. Rolling back to the previous build leaves the new tables unused; removing the crate simply reverts to in-memory behaviour.
- Any unexpected DB load can be mitigated by disabling the store wiring in a hotfix (the traits still tolerate `None`).

## Dependency Rationale

- Added `revaer-runtime` crate (internal) with `testcontainers` dev dependency to validate migrations against Postgres.
- No new third-party runtime dependencies beyond those already approved in the workspace.
