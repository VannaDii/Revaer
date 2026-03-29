# Indexer Instance Create Uses Definition Slug Key

- Status: Accepted
- Date: 2026-03-08
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had the API-surface rule requiring UUIDs or stable keys instead of internal primary keys.
  - The remaining indexer API violation was `IndexerInstanceCreateRequest`, which still accepted `indexer_definition_id`.
  - The public indexer catalog already exposes `upstream_slug`, so callers had a stable key available without exposing an internal database identifier.
- Decision:
  - Change indexer instance creation to accept `indexer_definition_upstream_slug` end to end.
  - Update the stored procedure wrapper and latest migration so runtime creation resolves definitions by slug instead of internal id.
  - Update handler, app-layer facade signatures, and API tests to use the slug key.
- Consequences:
  - The indexer API surface no longer requires callers to know an internal definition primary key.
  - Existing clients must send the slug field instead of the numeric id for instance creation.
  - The underlying database schema remains unchanged; only the procedure contract and API contract moved to the public key.
- Follow-up:
  - Keep checking new indexer endpoints for similar internal-PK leaks.
  - Revisit whether any multi-source future catalog needs `upstream_source + upstream_slug` as a composite public key.
