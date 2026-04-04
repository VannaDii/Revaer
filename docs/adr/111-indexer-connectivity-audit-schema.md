# Indexer connectivity and audit schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD defines connectivity snapshots, health events, and config audit logging.
  - These tables are prerequisites for health reporting and policy/action auditing.
- Decision:
  - Add indexer_connectivity_profile, indexer_health_event, and config_audit_log tables
    plus required enums for health events, connectivity status, and audit categories.
  - Enforce ERD constraints for success-rate bounds and audit entity references.
- Consequences:
  - Positive: schema supports connectivity rollups and durable audit trails.
  - Trade-off: rollup jobs and audit-writing procedures remain follow-up work.
- Follow-up:
  - Implement connectivity rollup job and health event emission per ERD.
  - Wire audit log writes in stored procedures and domain services.

## Task record

- Motivation:
  - Continue ERD implementation with connectivity and audit persistence.
- Design notes:
  - config_audit_log requires either a bigint PK or a public UUID per ERD notes.
  - indexer_connectivity_profile enforces error_class NULL for healthy status.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
