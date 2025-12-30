# Factory reset error context and allow-path validation

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: surface actionable factory reset failures in the UI and tighten allow-path validation for directory entries.
  - Constraints: preserve API i18n behavior, keep error context structured, and avoid new dependencies.
- Decision:
  - Derive the deepest error source string for factory reset failures and return it in structured context.
  - Validate each allow-path entry as a non-empty directory before persisting updates.
  - Add a unit test covering root error extraction.
- Consequences:
  - Positive outcomes: factory reset failures surface raw causes; invalid allow-path entries are rejected.
  - Risks or trade-offs: stricter validation can reject empty allow-path entries that previously slipped through.
- Follow-up:
  - Implementation tasks: confirm UI toasts surface context fields for factory reset failures.
  - Review checkpoints: run `just ci` and `just build-release` before handoff.

## Design notes
- Walk the `Error::source` chain to surface the innermost message without mutating the API detail string.

## Test coverage summary
- `just ci`: line coverage 80.04%.
- `just build-release`: succeeded.

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: allow-path validation rejects empty entries; factory reset error context exposes raw backend errors.
- Rollback: revert the validation change and error-context helper, remove the new test, then re-run `just ci`.
