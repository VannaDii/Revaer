# 001 – Global Configuration Revisioning

- Status: Proposed
- Date: 2025-02-23

## Context
- All runtime configuration must be hot-reloadable across multiple crates.
- Consumers need a consistent ordering guarantee for applying changes received via LISTEN/NOTIFY, with a fallback to polling.
- We require a DB-native mechanism that can be incremented from triggers without race conditions and that carries across deployments.

## Decision
- Introduce a singleton `settings_revision` table with an ever-incrementing `revision` counter.
- Wrap updates to configuration tables (`app_profile`, `engine_profile`, `fs_policy`, `auth_api_keys`, `query_presets`) in triggers that:
  1. Update `settings_revision.revision = revision + 1`.
  2. Emit `NOTIFY revaer_settings_changed, '<table>:<revision>:<op>'`.
- `ConfigService` exposes `ConfigSnapshot` to materialize a consistent view (revision + documents) for the application bootstrap path.
- The revision remains monotonic even if polling is used (consumers record the last seen revision and request deltas if they miss notifications).
- Mutation APIs validate payloads server-side, applying field-level type checks and respecting `app_profile.immutable_keys`. Violations surface as structured errors with section/field metadata, preventing silent drift.

## Consequences
- Multi-table updates executed inside a transaction surface as a single revision bump, preserving ordering for consumers.
- LISTEN subscribers that drop their connection can reconcile by reloading `settings_revision` and querying deltas > last_seen_revision.
- Trigger-level logic slightly increases write cost but keeps business code free of manual revision management.

## Follow-up
- Implement `apply_changeset` to write history rows with the associated revision.
- Add integration tests that exercise transactionally updating multiple tables and verifying a single revision increment.
