# 009 – FsOps Permission Hardening

- Status: Accepted
- Date: 2025-10-18

## Motivation

Phase One requires the filesystem pipeline to perform deterministic post-processing with metadata that survives restarts. The previous implementation only validated the library root and left extraction, flattening, transfer, and permission handling as TODOs. As a result, completed torrents could not be moved safely into the library, policies depending on chmod/chown/umask were ignored, and the orchestrator lacked the context to resume partially processed jobs.

## Design Notes

- `FsOpsService::apply` now accepts an explicit `FsOpsRequest` containing the torrent id, canonicalised source path, and the snapshot of the `FsPolicy`. The orchestrator resolves the source path from its catalog before invoking the pipeline.
- The pipeline executes deterministic stages (`validate_policy`, `allowlist`, `prepare_directories`, `compile_rules`, `locate_source`, `prepare_work_dir`, `extract`, `flatten`, `transfer`, `set_permissions`, `cleanup`, `finalise`) while persisting `.revaer.meta` after each critical transition. Resume attempts skip completed steps automatically.
- Extraction currently supports directory payloads and zip archives. Unsupported formats degrade the pipeline with a structured error and leave metadata untouched for later retries.
- The transfer step supports copy/move/hardlink semantics, records the chosen mode, and keeps destination metadata in-sync with the persisted record.
- Permission handling honours `chmod_file`, `chmod_dir`, `owner`, `group`, and `umask` directives. Unix platforms apply ownership changes using `nix::unistd::chown`; non-Unix targets reject ownership overrides with a descriptive error to avoid silent drift.
- Cleanup enforces `cleanup_keep`/`cleanup_drop` glob rules (including the `@skip_fluff` preset) and reports how many artefacts were removed.
- Errors mark the FsOps health component as degraded and emit `FsopsFailed` events; successful reruns clear the health flag and emit `FsopsCompleted`.

## Dependency Rationale

- Added `nix` (`features = ["user", "fs"]`) to resolve system users/groups and call `chown` in a portable, audited fashion. Standard library support is limited to numeric ownership changes on Unix and is entirely absent on non-Unix platforms. Alternatives considered:
  - Calling `libc::chown` directly: rejected to maintain the repository’s "no unsafe" guarantee and avoid platform-specific shims.
  - Shelling out to `chown`: rejected due to portability concerns, lack of atomic error propagation, and difficulty capturing precise failures for telemetry.
  `nix` provides safe wrappers, clear error types, and minimal dependencies, aligning with the minimal-footprint policy.

## Test Coverage Summary

- `revaer-fsops` unit tests now exercise the full happy path, resume semantics, flattening, allow-list enforcement, and permission error propagation. The new tests wait on pipeline events instead of arbitrary sleeps to reduce flakiness.
- `revaer-app` orchestrator tests were updated to subscribe to FsOps events and assert completion/failure handling without relying on time-based guesses.
- `just ci` (fmt, lint, udeps, audit, deny, test, cov) runs clean with the stricter pipeline enabled.

## Observability Updates

- Each FsOps stage increments the `fsops_steps_total` metric with its status (started/completed/failed/skipped).
- Success and failure events now include richer detail strings (source, destination, permission modes, cleanup counts) to aid operators.
- The health component toggles between degraded/recovered based on pipeline outcomes, ensuring `/health/full` reflects the current FsOps status.

## Risk & Rollback Plan

- Metadata persistence keeps prior state, so a rollback simply restores the previous binary without corrupting output directories.
- Ownership adjustments are gated to Unix platforms. Operators running on other OSes receive actionable errors instead of partial changes.
- Unsupported archive formats cause the pipeline to fail early without modifying destination directories, making forward fixes safe to deploy incrementally.
