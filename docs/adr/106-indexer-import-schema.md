# Indexer import schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD defines import_job and import_indexer_result for Prowlarr migration tracking.
  - Import jobs depend on search profiles and Torznab instances.
- Decision:
  - Add import_job and import_indexer_result tables with supporting enums.
  - Preserve ERD constraints for identifiers, status, and optional error/detail fields.
- Consequences:
  - Positive: schema supports import tracking and per-indexer outcomes.
  - Trade-off: import procedures and validation remain a follow-up step.
- Follow-up:
  - Implement import stored procedures and validation rules per ERD.
  - Add indexer-instance linkage rules and dry-run handling in procedures.

## Task record

- Motivation:
  - Continue ERD implementation with import pipeline persistence.
- Design notes:
  - import_indexer_result.indexer_instance_id remains a nullable bigint with no FK, per ERD.
  - import_job stores target profile/torznab references for later procedure enforcement.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
