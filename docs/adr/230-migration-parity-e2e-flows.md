# 230: Migration parity E2E flow coverage

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD acceptance requires end-to-end verification for Prowlarr import plus Torznab parity and download flows.
  - Existing API E2E coverage was split across specs and did not provide a single parity-flow assertion path.
- Decision:
  - Add a dedicated API E2E spec that exercises migration parity flows together:
    - Torznab caps/search parity semantics.
    - Torznab download auth/missing-source behavior.
    - Prowlarr API and backup import job run paths with dry-run setup.
- Consequences:
  - Positive outcomes:
    - ERD migration parity checks are exercised explicitly in one E2E flow.
    - Regression detection improves for cross-surface import + Torznab behavior.
  - Risks or trade-offs:
    - Slightly longer API E2E runtime due to additional scenario setup.
- Follow-up:
  - Extend this scenario to include successful Torznab download redirects once canonical source fixtures are available through public APIs.

## Motivation
- Close the checklist gap for explicit end-to-end migration parity coverage.

## Design notes
- Reused existing API fixtures and auth modes.
- Kept assertions deterministic around currently available endpoints and fixture-free behavior.

## Test coverage summary
- Added `tests/specs/api/indexers-migration-parity.spec.ts`.

## Observability updates
- No telemetry changes; this is E2E coverage only.

## Risk & rollback plan
- If endpoint semantics change, update this test alongside handler/service changes.
- Roll back by reverting the spec and checklist/ADR updates.

## Dependency rationale
- No new dependencies added.
