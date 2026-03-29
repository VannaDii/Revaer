# 191: Rate limit state purge test

- Status: Accepted
- Date: 2026-02-06
- Context:
  - ERD rules require rate_limit_state to purge minute buckets older than six hours.
  - The purge job existed but lacked a focused test to confirm retention behavior.
- Decision:
  - Add a stored-proc test that inserts old and recent rate_limit_state rows and verifies purge behavior.
  - Mark the ERD checklist item for rate_limit_state purging as complete once coverage is in place.
- Consequences:
  - Positive outcomes:
    - Retention behavior is enforced in CI for rate_limit_state cleanup.
    - Prevents regressions that could bloat rate_limit_state or delete fresh buckets.
  - Risks or trade-offs:
    - Slightly longer indexer job test runtime due to extra database setup.
- Follow-up:
  - Add more retention tests if additional job runners adopt similar cleanup rules.

## Motivation
- Ensure ERD-mandated rate limit retention rules are verified by stored-proc tests.
- Provide deterministic coverage for the six-hour purge window.

## Design notes
- Inserted two rate_limit_state rows with window_start older and newer than six hours.
- Verified that job_run_rate_limit_state_purge removes only the stale row.

## Test coverage summary
- Added a targeted job runner test in `revaer-data` for rate_limit_state purging.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- No new telemetry or metrics changes.

## Risk & rollback plan
- If retention windows change, update the test timestamps to match the new rules.
- Roll back by reverting the added test and checklist update if needed.

## Dependency rationale
- No new dependencies added.
