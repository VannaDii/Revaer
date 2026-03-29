# 188: Indexer search pages API

- Status: Accepted
- Date: 2026-02-06
- Context:
  - Search request creation exists, but there is no API surface to read sealed pages or page contents.
  - ERD requires stable page ordering and sealed page boundaries for streaming results.
  - All runtime DB reads must go through stored procedures with constant error messages.
- Decision:
  - Added stored procedures to list pages and fetch page items with stable ordering and page metadata.
  - Exposed v1 REST endpoints to list pages and fetch a specific page for a search request.
  - Updated API models and OpenAPI to document the new search page responses.
- Consequences:
  - Positive outcomes:
    - Clients can poll page lists and fetch sealed pages with deterministic ordering.
    - Page metadata (sealed_at, item_count) is exposed consistently across API and DB layers.
  - Risks or trade-offs:
    - Adds additional DB queries per page fetch (page metadata + items in a single proc).
    - UI still needs follow-on work to provide streaming UX, but the API is now available.
- Follow-up:
  - Add SSE notifications for new sealed pages once orchestration emits search result events.
  - Extend UI to consume search page endpoints and surface streaming updates.

## Motivation
- Provide a concrete API to read search request pages and support streaming UI flows.
- Keep read paths aligned with ERD page sealing and append-only ordering guarantees.

## Design notes
- Implemented `search_page_list_v1` and `search_page_fetch_v1` stored procedures with actor auth checks.
- Page fetch returns page metadata and items in one query, ensuring deterministic ordering by page position.
- Service layer maps stored-proc rows to API DTOs without exposing internal IDs.
- Patched `search_request_create_v1` policy snapshot lookup to avoid ambiguous `snapshot_hash` resolution when a snapshot already exists.
- Qualified `search_request_id` in `search_request_create_v1` inserts to avoid column/variable ambiguity during returns.
- Qualified `search_page_fetch_v1` lookups to avoid `sealed_at` output column ambiguity in PL/pgSQL.

## Test coverage summary
- Added stored-proc tests for page listing, invalid page numbers, and empty page fetches.
- Added handler tests for list and fetch responses plus error mapping.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- Reused existing request spans; no new metrics added for page reads.

## Risk & rollback plan
- If page fetch semantics need adjustment, update `crates/revaer-data/migrations/0079_indexer_search_pages.sql` and regenerate data wrappers.
- Roll back by reverting this ADR and the search page API routes if clients observe regressions.

## Dependency rationale
- No new dependencies added.
