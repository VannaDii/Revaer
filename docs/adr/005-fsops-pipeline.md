# 005 – FsOps Pipeline Hardening

- Status: Accepted
- Date: 2025-10-17

## Context
- Phase One promotes filesystem post-processing from a best-effort helper to a first-class workflow with explicit health semantics.
- The orchestrator must ensure every completed torrent flows through a deterministic FsOps state machine, emitting structured telemetry and reconciling mismatches with persisted metadata.
- Operators require visibility into FsOps latency, failures, and guard-rail breaches (e.g., missing extraction tools, permission errors) via `/health/full`, Prometheus, and the shared EventBus.

## Decision
- FsOps responsibilities live inside `revaer-fsops`, invoked by the orchestrator (`TorrentOrchestrator::apply_fsops`) with an explicit `FsOpsRequest` that carries the torrent id, resolved source path, and effective policy snapshot whenever a `Completed` event surfaces.
- Each pipeline step (`extract`, `flatten`, `transfer`, `set_permissions`, `cleanup`, `finalise`) records start/completion/failure events and increments Prometheus counters via `Metrics::inc_fsops_step`; the extraction stage currently focuses on zip archives and gracefully skips when inputs are already directories.
- Metadata is persisted alongside `.revaer.meta` to reconcile selection overrides and resume directories across restarts; mismatches trigger `SelectionReconciled` events plus guard-rail telemetry.
- Health degradation is published when FsOps detects latency guard rails, missing tools, or unrecoverable IO errors; recovery clears the `fsops` component from the degrade set.

## Consequences
- FsOps execution becomes observable and retry-friendly, enabling operator runbooks to diagnose stuck jobs with concrete metrics and events while capturing chmod/chown/umask outcomes in recorded metadata.
- Pipeline regressions now fail CI thanks to targeted unit/integration tests under `revaer-fsops` and orchestrator-level tests driving the shared event bus.
- The orchestration layer remains single-owner of FsOps invocation, simplifying future extensions (e.g., checksum verification, media tagging) without leaking concerns into the API.

## Verification
- `just test` exercises FsOps unit cases, while orchestrator integration tests validate event emission, degradation flows, and metadata reconciliation.
- `/health/full` and Prometheus snapshots display FsOps metrics during the runbook, confirming latency guard rails and failure counters behave as expected.
