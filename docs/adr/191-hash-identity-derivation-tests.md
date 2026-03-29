# 190: Hash identity derivation tests

- Status: Accepted
- Date: 2026-02-06
- Context:
  - ERD hash identity rules require deterministic magnet hash derivation with a strict precedence order.
  - Stored-proc behavior existed but lacked focused tests to lock in the precedence and normalization rules.
- Decision:
  - Add stored-proc tests for `derive_magnet_hash` to confirm infohash v2 precedence, infohash v1 fallback, and magnet URI normalization.
  - Mark the ERD checklist item for hash identity derivation as complete once coverage is in place.
- Consequences:
  - Positive outcomes:
    - Hash derivation precedence and normalization rules are exercised directly in CI.
    - Future regressions in magnet hash derivation will surface quickly.
  - Risks or trade-offs:
    - Minor increase in normalization test runtime.
- Follow-up:
  - Extend normalization coverage if additional hash identity rules are added.

## Motivation
- Ensure ERD-mandated hash identity rules are verified by stored-proc tests.
- Lock in precedence for infohash v2, infohash v1, and normalized magnet inputs.

## Design notes
- Added normalization tests that compare derivation results across input combinations.
- Verified that normalized and raw magnet URIs produce identical hash outputs when no infohash is present.

## Test coverage summary
- Added three `derive_magnet_hash` tests in `revaer-data` normalization helpers.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- No new telemetry or metrics changes.

## Risk & rollback plan
- If derivation logic changes, update the tests to match the new rule set.
- Roll back by reverting the normalization tests and checklist update if needed.

## Dependency rationale
- No new dependencies added.
