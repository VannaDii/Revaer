# Indexer query indexes

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD_INDEXERS.md defines a query-path index matrix for search, scoring, and
    telemetry workflows.
  - Many indexes are non-unique and must be added after table creation.
- Decision:
  - Add migration 0031_indexer_query_indexes.sql to create the ERD-specified
    non-unique indexes and partial indexes.
- Consequences:
  - Query paths have explicit index coverage for search, scoring, and retention.
  - Unique constraints continue to cover duplicate index requirements.
- Follow-up:
  - Revisit index coverage when stored procedures and query plans land.

## Task record

- Motivation:
  - Provide the ERD index matrix required for search and telemetry queries.
- Design notes:
  - Skipped indexes already covered by PK/UQ constraints.
  - Added partial indexes for sparse hash lookups and job scheduling.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0031 if index choices need revision.
- Dependency rationale:
  - No new dependencies added.
