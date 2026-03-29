# Indexer table/constraint alignment verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - ERD_INDEXERS.md requires all indexer tables, columns, defaults, CHECKs, UQ, and FK
    constraints to match the specification.
  - Verification found the policy_set.created_for_search_request_id FK missing after
    search_request was introduced.
  - Auto-created request policy sets must be purged with their search_request per ERD notes.
- Decision:
  - Add a migration to enforce the policy_set.created_for_search_request_id FK with
    ON DELETE CASCADE to align with the ERD.
  - Record the table/column/FK parity verification against ERD tables and migrations.
- Consequences:
  - Positive: referential integrity matches ERD and search retention cascades to
    auto-created policy sets.
  - Risk: existing orphaned policy_set rows would block the migration.
- Follow-up:
  - Continue validating per-table Notes invariants and add tests where appropriate.

## Task record

- Motivation:
  - Close the remaining schema gap so all ERD indexer tables and FKs match the spec.
- Design notes:
  - Added FK policy_set.created_for_search_request_id -> search_request.search_request_id
    with ON DELETE CASCADE in migration 0068.
  - Verified ERD table list, column coverage, and FK presence against migrations.
- Test coverage summary:
  - just ci
  - just ui-e2e
- Observability updates:
  - None.
- Risk & rollback plan:
  - If the FK fails due to orphaned policy_set rows, drop the constraint and backfill
    or null invalid references before reapplying.
- Dependency rationale:
  - No new dependencies.
