# Indexer conflict and decision schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md conflict tracking and filter decision tables.
  - Preserve auditability of metadata conflicts and policy filtering outcomes.
- Decision:
  - Add migration 0025_indexer_conflicts_decisions.sql for conflicts and decisions.
  - Introduce enums for conflict and decision types with ERD-aligned values.
- Consequences:
  - Conflict resolution workflows can be recorded with an audit trail.
  - Search filter decisions can be persisted for transparency and debugging.
- Follow-up:
  - Add outbound_request_log, user actions, acquisition, reputation, and job tables.
  - Implement stored procedures for conflict resolution and filtering decisions.

## Task record

- Motivation:
  - Capture durable metadata conflicts and policy decisions per ERD_INDEXERS.md.
- Design notes:
  - Kept constraints minimal and aligned with ERD requirements for nullable references.
  - Search filter decisions require at least one canonical reference.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0025 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
