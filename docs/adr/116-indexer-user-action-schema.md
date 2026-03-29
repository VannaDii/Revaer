# Indexer user action and acquisition schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md feedback and acquisition tracking tables.
  - Persist user actions and download attempts for ranking and reputation signals.
- Decision:
  - Add migration 0026_indexer_user_actions.sql for user actions and acquisition attempts.
  - Introduce enums for actions, reasons, acquisition status/origin/failure, and client names.
- Consequences:
  - User feedback and acquisition events are stored with constrained identifiers.
  - Future reputation rollups can rely on acquisition data.
- Follow-up:
  - Add outbound_request_log, reputation, and job scheduling tables.
  - Implement stored procedures and ingestion paths for acquisitions.

## Task record

- Motivation:
  - Capture user interactions and download outcomes per ERD_INDEXERS.md.
- Design notes:
  - Enforced identifier presence and failure-class rules on acquisition_attempt.
  - User action metadata stored as key/value with unique keys per action.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0026 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
