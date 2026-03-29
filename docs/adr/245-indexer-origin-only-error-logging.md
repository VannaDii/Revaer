# 245. Indexer origin-only error logging

- Status: accepted
- Date: 2026-03-16

## Motivation

- `ERD_INDEXERS_CHECKLIST.md` still leaves the origin-only error logging rules unchecked even though the indexer stack already carries structured `code` and `sqlstate` fields through typed errors.
- `crates/revaer-app/src/indexers.rs` was re-logging propagated `DataError` values while also converting them into service errors, which duplicated origin logs and violated `AGENTS.md`.
- The next efficient step is to make the app-layer mapper functions pure translations so origin logs remain singular while callers still receive stable service error kinds and structured context.

## Design notes

- Remove `tracing::error!` side effects from the indexer service error-mapper helpers in `crates/revaer-app/src/indexers.rs`.
- Keep the existing mapping taxonomy unchanged so service callers still receive the same `kind`, `code`, and `sqlstate` values.
- Add mapper coverage proving structured error context survives translation without requiring logging side effects.

## Test coverage summary

- Added a unit test covering representative mapper paths for definition, tag, and indexer-field errors.
- The new assertions verify `kind`, `code`, and `sqlstate` preservation for propagated stored-procedure failures.
- Full repository quality gates remain the final verification for regression safety.

## Observability updates

- No new emitters were added.
- This change reduces duplicate logs by keeping error emission at the actual failure origin while preserving structured context on returned service errors.

## Risk & rollback plan

- Risk is low because the change is limited to log side effects in app-layer error translation.
- If diagnostics regress, rollback is a straight revert of the mapper cleanup and accompanying checklist/task-record updates.

## Dependency rationale

- No new dependencies were added.
