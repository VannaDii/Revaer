# 227: Search zero-result explainability

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD acceptance requires zero-result searches to expose why nothing was returned.
  - Existing search page APIs returned pages/items only, without skipped/blocked/rate-limit diagnostics.
- Decision:
  - Add stored procedures `search_request_explainability_v1` and `search_request_explainability`.
  - Extend `SearchPageListResponse` with an `explainability` object that reports:
    - zero runnable indexers
    - skipped canceled/failed indexers
    - blocked result count and blocking rule IDs
    - rate-limited and retrying indexer counts
  - Wire the new procedure through `revaer-data`, `revaer-app`, and API handlers.
- Consequences:
  - Positive outcomes:
    - UI/API callers can explain “nothing found” states with structured diagnostics.
    - Explainability semantics are enforced through stored-proc and handler tests.
  - Risks or trade-offs:
    - Response payload size increases slightly for page list calls.
- Follow-up:
  - Expose these explainability fields in the UI once indexer search pages are integrated in the frontend route.

## Motivation
- Ensure zero-result states are actionable instead of silent, matching ERD acceptance rules.

## Design notes
- Kept runtime SQL policy compliant by introducing stored procedures instead of ad-hoc queries.
- Reused `search_page_list_v1` authorization/visibility checks in the explainability procedure to preserve error semantics.
- Counted blocked results from `search_filter_decision` decisions (`drop_source`, `drop_canonical`).

## Test coverage summary
- Added `revaer-data` tests for explainability defaults and blocked/rate-limited/retrying states.
- Updated API handler test support and search page handler tests for the new response shape.

## Observability updates
- No new spans/metrics; this feature surfaces existing run/filter state via API responses.

## Risk & rollback plan
- If semantics need adjustment, update the procedure outputs and response mapping together.
- Roll back by reverting migration + API model/service wiring if clients cannot adopt the additive field.

## Dependency rationale
- No new dependencies added.
