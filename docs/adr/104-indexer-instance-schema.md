# Indexer instance schema and RSS

- Status: Accepted
- Date: 2026-01-25
- Context:
  - Indexer instances, routing policies, and RSS schedules are required to configure
    real indexers and persist their operational state.
- Decision:
  - Add a migration that introduces indexer instance tables, routing policy tables,
    RSS tracking tables, and related enums.
  - Enforce ERD constraints (ranges, uniqueness, hash formats) via database checks.
- Consequences:
  - Positive: provides the durable schema for indexer configuration, tags, domains, and RSS.
  - Trade-off: requires additional migrations for imports, policies, and search flows.
- Follow-up:
  - Add import_job tables once search profiles and torznab instances exist.
  - Implement stored procedures and seed data for routing and instance management.

## Task record

- Motivation:
  - Continue ERD implementation with dependency-ready indexer instance tables.
- Design notes:
  - Routing policy is introduced to satisfy the FK from indexer_instance.
  - Hash columns enforce lowercase hex constraints to match global rules.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if downstream constraints change.
- Dependency rationale:
  - No new dependencies added.
