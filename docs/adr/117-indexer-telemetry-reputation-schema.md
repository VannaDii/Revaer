# Indexer telemetry and reputation schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md telemetry logging and reputation rollups.
  - Ensure outbound request invariants are enforced at the schema layer.
- Decision:
  - Add migration 0027_indexer_telemetry_reputation.sql for outbound_request_log and source_reputation.
  - Introduce enums for request types, outcomes, mitigations, and reputation windows.
- Consequences:
  - Connectivity and reputation rollups can rely on consistent telemetry inputs.
  - Rate-limited and success/failure invariants are enforced in the database.
- Follow-up:
  - Add job scheduling tables and stored procedures for rollups and retention.
  - Implement index coverage for telemetry and reputation queries.

## Task record

- Motivation:
  - Capture outbound request telemetry and reputation rollups per ERD_INDEXERS.md.
- Design notes:
  - Enforced outcome/error-class invariants and numeric ranges for rates.
  - Added defaults for timestamps to keep writes consistent.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0027 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
