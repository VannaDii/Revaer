# 143 Task: Indexer procedure fixes (RSS apply, base score refresh, normalization)

- Status: Accepted
- Date: 2026-01-27
- Context:
  - RSS poll apply failed under outer-join locking and returned non-domain errors.
  - Base score refresh queried a non-existent canonical_torrent_id on durable sources.
  - Title normalization regex boundaries did not strip resolution tokens consistently.
  - Import job status aggregation hit ambiguous column references.
  - Factory reset did not re-seed indexer defaults, causing tag operations to fail.
- Decision:
  - Patch stored procedures with targeted fixes and add a new migration to apply them.
  - Keep Rust wrappers aligned with enum/array casts and session config expectations.
  - Extend factory reset to reseed indexer defaults and system actor data.
- Consequences:
  - RSS poll apply now locks the subscription row without outer-join errors.
  - Base score refresh derives canonical/source pairs from context scores and recent sources.
  - Title normalization removes known release tokens reliably.
  - Import job status aggregation no longer fails on ambiguity.
  - Factory reset restores seed data needed for indexer tag operations.
- Follow-up:
  - Re-run full CI and UI E2E gates.
  - Monitor RSS apply logs for any unexpected lock contention.

## Motivation

Fix indexer data-layer regressions that caused RSS polling to fail before domain errors surfaced,
and align stored procedures with the canonical/source relationships defined in ERD_INDEXERS.md.

## Design notes

- Reworked `rss_poll_apply_v1` to lock only the subscription row (`FOR UPDATE OF sub`) and left
  other joins unlocked.
- Updated base-score refresh to use durable source recency plus context-score links for canonical
  mapping, keeping scoring inputs on `canonical_torrent_source`.
- Corrected `normalize_title_v1` regex boundaries and whitespace patterns using explicit escapes.
- Qualified `import_job_get_status_v1` result aggregation to avoid `status` ambiguity.
- Updated RSS apply wrapper casts and test-time secret config to match runtime expectations.

## Test coverage summary

- `just ci`
- `just ui-e2e`

## Observability updates

- None.

## Risk & rollback plan

- Risk: base-score refresh may skip canonicals without context-score links.
- Rollback: apply a follow-up migration restoring previous procedure bodies and revert wrapper
  changes if needed.

## Dependency rationale

- No new dependencies.
