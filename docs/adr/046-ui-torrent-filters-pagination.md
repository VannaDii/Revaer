# UI Torrent Filters, Pagination, and URL Sync

- Status: Accepted
- Date: 2025-12-24
- Context:
  - Motivation: expose torrent filters in the URL and support paged list loading without breaking the normalized store.
  - Constraints: reuse existing API query semantics, avoid new dependencies, and keep URL updates inside app-level routing.
- Decision:
  - Summary: parse/filter query params from the router location, update the URL when filters change, and add an explicit Load more flow that appends rows.
  - Design notes: use `build_torrent_filter_query` for URL-only filters, keep refresh fetches cursor-free, and append rows only when a cursor is provided for pagination.
  - Alternatives considered: store cursor in the URL or auto-load more on scroll; rejected to keep query stable and avoid hidden fetches.
- Consequences:
  - Positive outcomes: shareable filter URLs, explicit paging, and predictable list refresh behavior.
  - Risks/trade-offs: query sync relies on history replace semantics; overlapping API pages could still cause duplicate rows.
  - Observability updates: none.
- Follow-up:
  - Implementation tasks: wire filter inputs, add Load more, and append list reducer support.
  - Test coverage summary: added unit tests for query round-tripping and append-row behavior.
  - Dependency rationale: no new dependencies introduced.
  - Risk & rollback plan: revert filter URL sync and pagination append logic if list state becomes inconsistent.
