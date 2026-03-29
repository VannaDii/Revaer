# Indexer canonicalization schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Implement ERD_INDEXERS.md canonicalization tables for deduped torrents and durable sources.
  - Enforce hash identity and typed attribute invariants at the schema layer.
  - Keep enums and tables aligned with existing migrations and single-tenant scope.
- Decision:
  - Add migration 0022_indexer_canonicalization.sql to create canonical tables and enums.
  - Apply ERD validation rules for hashes, IDs, typed attributes, and identity strategies.
- Consequences:
  - Canonical torrent/source data is stored with enforced identity constraints.
  - Downstream search and ingest tables can reference canonical entities safely.
- Follow-up:
  - Implement search_request tables and ingestion stored procedures per ERD_INDEXERS.md.
  - Add remaining canonical scoring, conflict, and decision tables.

## Task record

- Motivation:
  - Establish canonical torrent and source storage to unblock search request ingestion.
- Design notes:
  - Enforce hash, ID, and typed attribute invariants directly in the schema.
  - Keep canonical tables aligned to ERD_INDEXERS.md and dependency order.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0022 if schema issues surface.
- Dependency rationale:
  - No new dependencies added.
