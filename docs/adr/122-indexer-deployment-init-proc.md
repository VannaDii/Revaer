# Indexer deployment initialization procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD_INDEXERS.md specifies deployment_init_v1 to bootstrap deployment defaults.
  - Initialization must be idempotent and enforce actor verification.
- Decision:
  - Add migration 0032_indexer_deployment_init.sql with deployment_init_v1 and
    stable wrapper deployment_init.
  - deployment_init_v1 enforces verified admin/owner actors and seeds defaults.
- Consequences:
  - Deployments can be initialized via stored procedure calls.
  - System defaults are re-applied safely when missing.
- Follow-up:
  - Implement the remaining stored procedures in Phase 5.

## Task record

- Motivation:
  - Provide the ERD-specified deployment initialization entry point.
- Design notes:
  - Procedure is idempotent and reuses existing seed helpers.
  - Authorization requires verified owner/admin actors.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0032 if procedure behavior needs revision.
- Dependency rationale:
  - No new dependencies added.
