# 247. Indexer backup and restore

- Status: accepted
- Date: 2026-03-18

## Motivation

- `ERD_INDEXERS_CHECKLIST.md` still left backup and restore of indexer settings open even though the admin console already exposed most of the underlying configuration entities.
- Operators needed a user-facing way to export the current indexer graph and re-apply it later without manually replaying tags, routing policies, rate limits, instance fields, and RSS settings.
- The next efficient step was to add a sanitized backup format and restore flow on top of the existing stored-procedure-backed write APIs instead of inventing a separate persistence path.

## Design notes

- Add stored-procedure-backed export reads that return normalized rows for tags, rate-limit policies, routing policies, and indexer instances with secret references but never secret plaintext.
- Assemble those flattened rows into a typed snapshot document in the app layer so the HTTP and UI layers can share a stable backup format.
- Add `/v1/indexers/backup/export` and `/v1/indexers/backup/restore` endpoints and wire `/indexers` with export and restore controls plus unresolved-secret feedback.
- Restore replays the existing create/update procedures and skips only secret bindings whose referenced secret is unavailable, surfacing them back to the operator for follow-up.

## Test coverage summary

- Added stored-procedure tests for the backup export wrappers in `revaer-data`.
- Added API handler coverage for backup export and restore success and error mapping.
- Extended the `/indexers` route smoke test to assert the new backup and restore panel renders.
- Full `just ci` and `just ui-e2e` remain the end-to-end verification gates.

## Observability updates

- Backup export and restore endpoints are traced through the existing HTTP span layer.
- The restore response includes unresolved secret-binding summaries so operators can distinguish successful object replay from missing-secret follow-up work.

## Risk & rollback plan

- The main risk is restore failure on deployments with conflicting names or missing referenced secrets; those conditions now fail fast or are surfaced explicitly instead of being silently ignored.
- Secret plaintext is intentionally excluded from exports, so rollback is a straightforward revert of the backup routes, snapshot models, and UI panel if the format proves insufficient.

## Dependency rationale

- No new dependencies were added.
