# Indexer soft-delete coverage verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires soft-delete support via `deleted_at` on specific indexer tables.
  - We need confirmation before expanding API and service layers that assume soft deletes.
- Decision:
  - Verify `deleted_at` exists on all required tables:
    - indexer_instance
    - routing_policy
    - policy_set
    - search_profile
    - tag
    - torznab_instance
    - rate_limit_policy
- Consequences:
  - Soft-delete semantics are available for indexer configuration entities.
  - API handlers can depend on `deleted_at` for active filtering.
- Follow-up:
  - Keep soft-delete requirements in mind for any new indexer-facing tables.

## Task record

- Motivation:
  - Confirm the ERD soft-delete rule is implemented consistently in migrations.
- Design notes:
  - Verified `deleted_at` columns in migrations 0012, 0014, 0016, 0018, and 0019.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future migrations omit `deleted_at`. Roll back by correcting the migration and
    re-running schema checks.
- Dependency rationale:
  - No new dependencies added.
