# UI Torrent List Updated Timestamp Column

- Status: Accepted
- Date: 2025-12-24
- Context:
  - Motivation: surface the last updated timestamp alongside the existing list columns.
  - Constraints: avoid new dependencies and keep row slices stable for list rendering performance.
- Decision:
  - Summary: store a formatted updated timestamp string in the torrent row base slice and render it as an optional column.
  - Alternatives considered: compute formatting in the component layer or add a relative time utility; rejected to keep row rendering pure and avoid new helpers.
- Consequences:
  - Positive outcomes: list rows now include an explicit updated timestamp column with overflow fallback.
  - Risks/trade-offs: updated timestamps refresh only when list data is refreshed, not on every SSE event.
  - Observability updates: none.
- Follow-up:
  - Implementation tasks: keep formatting consistent in the summary conversion.
  - Test coverage summary: added assertions for updated timestamps in row conversion tests.
  - Dependency rationale: no new dependencies introduced.
  - Risk & rollback plan: remove updated column mapping if list layout regresses.
