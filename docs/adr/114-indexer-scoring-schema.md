# Indexer scoring schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md scoring and best-source materialization tables.
  - Preserve context-specific ordering for search/profile/policy views.
- Decision:
  - Add migration 0024_indexer_scoring.sql for scoring and best-source tables.
  - Introduce context_key_type enum and enforce score range checks.
- Consequences:
  - Canonical sources can be ranked globally and per context.
  - Best-source tables are ready for refresh jobs and search paging.
- Follow-up:
  - Add conflicts and decision tables plus outbound log and reputation tracking.
  - Implement stored procedures that compute scores and refresh best-source rows.

## Task record

- Motivation:
  - Provide schema support for deterministic source ranking in searches.
- Design notes:
  - Enforced score ranges and uniqueness constraints per ERD_INDEXERS.md.
  - Context types are stored as a dedicated enum for clarity.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0024 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
