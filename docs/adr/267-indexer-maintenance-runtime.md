# Indexer maintenance runtime

- Status: Accepted
- Date: 2026-04-03
- Context:
  - Branch analysis against `ERD_INDEXERS.md` reopened a real runtime gap: indexer maintenance jobs existed as stored procedures but the Revaer server process was not actually claiming and executing them on cadence.
  - The ERD requires in-process scheduling for retention, connectivity, reputation, canonical upkeep, policy cleanup, rate-limit cleanup, and RSS-adjacent maintenance rather than relying on external cron.
  - The same review also confirmed that live manual search, Torznab search execution, RSS HTTP polling, and runtime import executors are still separate unresolved gaps and should not be silently conflated with maintenance scheduling.
- Decision:
  - Add a dedicated injected `indexer_runtime` module in `revaer-app` that owns a small Tokio loop and executes due maintenance jobs through stored-proc wrappers.
  - Keep the runtime testable with an internal backend trait so bootstrap remains the only place constructing concrete collaborators.
  - Add a missing stored-proc wrapper for `canonical_prune_low_confidence` so the runtime can advance `job_schedule` consistently for that job class as well.
- Consequences:
  - The server now advances maintenance job cadence in-process for retention, connectivity refresh, reputation rollups, canonical backfill/prune, policy GC/repair, rate-limit purge, and RSS subscription backfill.
  - Telemetry now records per-job success, failure, and skip outcomes from the runtime loop using existing indexer job counters/histograms.
  - This does not close the separate executor gaps for live search, Torznab fetches, RSS outbound polling, or Prowlarr import execution; those remain open checklist items.
- Follow-up:
  - Implementation tasks:
    - Wire live RSS/search/import executors into the remaining runtime lanes.
    - Extend acceptance coverage from maintenance-loop unit coverage to live end-to-end execution parity.
  - Review checkpoints:
    - `just ci`
    - `just ui-e2e`

