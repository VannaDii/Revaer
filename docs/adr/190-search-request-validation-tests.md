# 189: Search request validation tests

- Status: Accepted
- Date: 2026-02-06
- Context:
  - Search request creation enforces identifier, season/episode, and category validation rules in stored procedures.
  - Validation paths were under-tested, leaving ERD rule coverage uncertain.
- Decision:
  - Add stored-proc tests that exercise identifier mismatch, torznab season/episode validation, and invalid category filters.
  - Mark the ERD validation checklist item as complete once coverage is in place.
- Consequences:
  - Positive outcomes:
    - Validation rules are exercised directly against stored procedures.
    - Future regressions in search request validation will fail fast in CI.
  - Risks or trade-offs:
    - Slightly longer indexer test runtime due to additional database cases.
- Follow-up:
  - Extend validation tests as new rules are added to `search_request_create_v1`.

## Motivation
- Ensure ERD-mandated validation rules are enforced and verified in CI.
- Provide deterministic stored-proc coverage for identifier, torznab, and category filter rules.

## Design notes
- Added stored-proc tests for identifier mismatch, torznab season/episode validation, and invalid category filters.
- Kept tests aligned with existing error code taxonomy and `DataError` mapping.
- Fixed `search_request_create_v1` to compare `query_type` and `identifier_type` via text casts to avoid enum type mismatch errors.

## Test coverage summary
- Added three new `search_request_create` validation tests in `revaer-data`.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- No new telemetry or metrics changes.

## Risk & rollback plan
- If validations evolve, update tests to match new error codes or rules.
- Roll back by reverting the added tests and checklist update if needed.

## Dependency rationale
- No new dependencies added.
