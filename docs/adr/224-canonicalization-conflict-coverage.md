# 224: Canonicalization conflict coverage

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD canonicalization rules require preserving durable source identity and logging conflicts when incoming hashes disagree.
  - Existing tests covered fallback identities and size rollups, but not explicit hash-conflict logging behavior.
- Decision:
  - Add a stored-procedure test for `search_result_ingest` that ingests two rows for the same source GUID with conflicting hashes.
  - Verify durable hash immutability, `source_metadata_conflict` logging, and `indexer_health_event` emission.
  - Mark the canonicalization-rule checklist item complete after coverage is in place.
- Consequences:
  - Positive outcomes:
    - Conflict-handling behavior is verified directly against stored procedures in CI.
    - Regressions that overwrite durable identities or skip conflict audit signals fail fast.
  - Risks or trade-offs:
    - Slight additional runtime in `revaer-data` tests.
- Follow-up:
  - Extend conflict tests for tracker/external-id mismatches if rules change.

## Motivation
- Ensure ERD-required conflict handling is exercised, not just identity selection and size rollups.

## Design notes
- Added a targeted `search_result_ingest` test that reuses the same durable source and injects a conflicting `infohash_v1`.
- Assertions cover three outputs: durable hash remains original, conflict row recorded with `conflict_type=hash`, and health event is emitted.

## Test coverage summary
- Added one `search_result_ingest` conflict test in `revaer-data`.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- No new metrics/spans; verified existing `indexer_health_event` signal behavior.

## Risk & rollback plan
- If canonicalization conflict semantics evolve, update expected conflict/audit values in the test.
- Roll back by reverting the test and checklist/ADR updates if needed.

## Dependency rationale
- No new dependencies added.
