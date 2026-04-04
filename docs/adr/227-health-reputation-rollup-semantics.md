# 226: Health and reputation rollup semantics from outbound logs

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD indexer acceptance requires connectivity and reputation statistics to follow `outbound_request_log` semantics.
  - Existing job tests covered base rollup behavior but did not explicitly assert `rate_limited` exclusion from sample counts.
- Decision:
  - Add a stored-procedure test for job rollups that mixes successful, non-rate-limited failures, and rate-limited failures.
  - Verify both connectivity and reputation calculations exclude `error_class=rate_limited` samples.
  - Mark the corresponding ERD acceptance checklist item complete.
- Consequences:
  - Positive outcomes:
    - Connectivity (`indexer_connectivity_profile.success_rate_1h`) and reputation (`source_reputation.request_*`) rules are now pinned to ERD semantics in CI.
    - Regression risk around sample-count inflation from rate-limited rows is reduced.
  - Risks or trade-offs:
    - Slightly longer `revaer-data` test runtime.
- Follow-up:
  - Extend sampling tests if ERD expands included request types beyond current coverage.

## Motivation
- Enforce that health/reputation rollups remain authoritative to `outbound_request_log` semantics, especially rate-limit exclusions.

## Design notes
- Added `connectivity_and_reputation_exclude_rate_limited_samples` in `revaer-data` job tests.
- Test inserts 30 successes, 5 non-rate-limited failures, and 10 rate-limited failures, then validates:
  - `success_rate_1h = 30/35` (not `30/45`)
  - `request_count = 35` and `request_success_count = 30`

## Test coverage summary
- Added one stored-procedure rollup test in `crates/revaer-data/src/indexers/jobs.rs`.
- Full gates (`just ci`, `just ui-e2e`) are run before hand-off.

## Observability updates
- No new telemetry surface added; this validates existing derived-health semantics.

## Risk & rollback plan
- If rollup semantics change, update the expected sample math and checklist references.
- Roll back by reverting this ADR, checklist line, and test if requirement scope changes.

## Dependency rationale
- No new dependencies added.
