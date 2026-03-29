# Indexer job schedule schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md job scheduling table and enum constraints.
  - Align cadence and jitter bounds with runtime scheduler expectations.
- Decision:
  - Add migration 0028_indexer_jobs.sql for job_key enum and job_schedule.
  - Enforce cadence range and jitter bounds per ERD notes.
- Consequences:
  - Scheduler state is stored in a single table with clear invariants.
  - Deployment seeding must populate required job rows.
- Follow-up:
  - Add deployment seed procedures for job_schedule rows.
  - Implement job_claim_next_v1 and job completion updates.

## Task record

- Motivation:
  - Establish job scheduling primitives required for indexer retention and rollups.
- Design notes:
  - cadence_seconds constrained to 30..604800 per ERD.
  - jitter_seconds constrained to 0..cadence_seconds.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0028 if scheduler constraints need adjustment.
- Dependency rationale:
  - No new dependencies added.
