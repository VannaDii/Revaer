# 186: Indexer search requests API and allocation guard refinements

- Status: Accepted
- Date: 2026-02-04
- Context:
  - Add v1 REST endpoints for indexer search request create/cancel while keeping stored-procedure boundaries.
  - Address PR feedback on allocation safety and test-only helper isolation.
  - Ensure allocation guards use live memory data and cap single allocations at 80% of available memory.
- Decision:
  - Added search request create/cancel request/response models plus API handlers, routes, and facade wiring.
  - Added allocation helpers that check live memory availability and use checked capacity reservations before dynamic allocations.
  - Tightened test helpers to use bounded body reads and explicit error parsing.
- Consequences:
  - Positive outcomes:
    - Search request orchestration is now reachable via v1 REST endpoints.
    - Allocation checks are centralized and consistently enforced with live memory data.
  - Risks or trade-offs:
    - Requests with large payloads may be rejected under memory pressure.
    - Additional validation and allocation checks add small overhead to hot paths.
- Follow-up:
  - Implement remaining search request lifecycle endpoints (list/status) as the checklist advances.
  - Keep UI/E2E coverage aligned as new search request surfaces are added.

## Motivation
- Provide a REST API for indexer search requests to unblock UI/CLI orchestration.
- Align allocation safeguards with GHAS feedback and operational safety goals.

## Design notes
- Handlers trim and normalize request inputs, translate service errors into RFC9457 responses, and delegate to stored-proc backed services.
- Allocation checks rely on live memory snapshots and cap single allocations at 80% of available memory.

## Test coverage summary
- Added/updated unit tests for search request handlers and allocation helpers.
- Ran `just ci` and `just ui-e2e` to validate unit, integration, coverage, and E2E suites.

## Observability updates
- No new metrics added; existing request spans and error contexts remain in place.

## Risk & rollback plan
- If allocation checks prove too strict, adjust the limit in `crates/revaer-api/src/http/handlers/indexers/allocation.rs`.
- Roll back by reverting this ADR and associated handler changes if endpoints regress.

## Dependency rationale
- No new dependencies added; re-used existing `systemstat` memory probing.
