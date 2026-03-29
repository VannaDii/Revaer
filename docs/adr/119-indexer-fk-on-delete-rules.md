# Indexer FK on-delete rules

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD_INDEXERS.md requires cascade deletes from indexer_instance to instance children.
  - Some FKs were created without explicit on-delete behavior.
- Decision:
  - Add migration 0029_indexer_fk_rules.sql to enforce cascading FKs for
    indexer_instance child tables.
- Consequences:
  - Hard-deleting an indexer_instance will cascade to dependent config and
    diagnostics rows.
  - Soft-delete behavior remains unchanged.
- Follow-up:
  - Review remaining FK behaviors as stored procedures are introduced.

## Task record

- Motivation:
  - Align schema with ERD on-delete rules for indexer_instance children.
- Design notes:
  - Replaced default FK constraints with ON DELETE CASCADE on instance child tables.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0029 if cascading rules need adjustment.
- Dependency rationale:
  - No new dependencies added.
