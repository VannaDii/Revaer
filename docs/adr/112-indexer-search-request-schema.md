# Indexer search request schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md search request tables, enums, and streaming page state.
  - Enforce search/run state invariants and observation typing at the schema layer.
- Decision:
  - Add migration 0023_indexer_search_requests.sql to create search request tables and enums.
  - Apply ERD validation rules for status transitions, cursors, and observation attributes.
- Consequences:
  - Search request storage is ready for ingestion and paging flows.
  - Downstream procedures can rely on schema checks for state integrity.
- Follow-up:
  - Add canonical scoring, conflict tracking, and job tables per ERD_INDEXERS.md.
  - Implement stored procedures for search orchestration and ingestion.

## Task record

- Motivation:
  - Land the search request schema needed for streaming and run tracking.
- Design notes:
  - Enforced status/timestamp and attribute typing checks per ERD_INDEXERS.md.
  - Kept enums scoped to current table usage to avoid unused schema items.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0023 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
