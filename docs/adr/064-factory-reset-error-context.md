# Factory reset hardening and allow-path validation

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: surface actionable factory reset failures, prevent long-running resets from hanging, and tighten allow-path validation for directory entries.
  - Constraints: preserve API i18n behavior, keep error context structured, and avoid new dependencies or inline SQL outside migrations.
- Decision:
  - Derive the deepest error source string for factory reset failures and return it in structured context.
  - Allow factory resets to proceed without API keys when no keys exist, even if a stale API key header is present.
  - Add a lock timeout in the factory reset stored procedure to avoid indefinite blocking.
  - Validate each allow-path entry as a non-empty directory before persisting updates.
  - Add unit tests covering error extraction and the stale API key path.
- Consequences:
  - Positive outcomes: factory reset failures surface raw causes; invalid allow-path entries are rejected; resets fail fast on lock contention.
  - Risks or trade-offs: stricter validation can reject empty allow-path entries that previously slipped through; lock timeouts may require retrying during heavy database activity.
- Follow-up:
  - Implementation tasks: confirm UI toasts surface context fields for factory reset failures and lock timeouts.
  - Review checkpoints: run `just ci` and `just build-release` before handoff.

## Design notes
- Walk the `Error::source` chain to surface the innermost message without mutating the API detail string.

## Test coverage summary
- `just ci`: line coverage 80.06%.
- `just build-release`: succeeded.

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: allow-path validation rejects empty entries; factory reset error context exposes raw backend errors; lock timeout may surface new transient failures during heavy DB activity.
- Rollback: revert the allow-path validation, auth fallback, and lock-timeout adjustments, remove the related tests, then re-run `just ci`.
