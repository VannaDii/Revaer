# Indexer job runner follow-up procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Job runner needs procedures for policy snapshot GC, refcount repair, and rate limit state purge.
  - These procedures are used by scheduled jobs and must align with ERD retention rules.
- Decision:
  - Add job runner procedures for policy snapshot GC, refcount repair, and rate limit state purge.
  - Keep wrappers without version suffix to preserve stable entry points.
- Consequences:
  - Policy snapshot rows with ref_count=0 will be purged after 30 days.
  - Refcount repair can correct drift between snapshots and active searches.
  - Rate limit state rows older than 6 hours are cleaned up.
- Follow-up:
  - Implement remaining job runner procedures (RSS poll/backfill, connectivity refresh, reputation rollup).

## Task record

- Motivation:
  - Close remaining ERD-required job runner procedures that do not depend on external systems.
- Design notes:
  - Refcount repair uses search_request counts as source of truth.
  - GC window is fixed at 30 days per ERD; rate limit purge uses 6-hour cutoff.
- Test coverage summary:
  - Not yet added; requires SQL harness coverage for refcount updates and purge cutoffs.
- Observability updates:
  - None in this change (DB-only procedures).
- Risk & rollback plan:
  - Risk: accidental over-purge if cutoff logic is wrong. Rollback by reverting migration 0051.
- Dependency rationale:
  - No new dependencies added.
