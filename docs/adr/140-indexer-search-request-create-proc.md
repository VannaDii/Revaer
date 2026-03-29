# Indexer search request create procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Search requests must validate identifiers, torznab modes, and category filters per the ERD.
  - Policy snapshots must be reusable with deterministic hashing and rule ordering.
- Decision:
  - Add `search_request_create_v1` with request validation, policy snapshot materialization, category/domain intersection, and runnable indexer gating.
  - Return both `search_request_public_id` and the request policy set public id for downstream orchestration.
- Consequences:
  - Search requests short-circuit to finished when domain/allowlist constraints or policy allowlists eliminate all runnable indexers.
  - Invalid identifier or category combinations fail fast with explicit error codes.
- Follow-up:
  - Implement `search_result_ingest_v1` and canonical maintenance procedures.
  - Add SQL harness tests for search_request creation paths and edge cases.

## Task record

- Motivation:
  - Enable search request creation with ERD-compliant validation, policy snapshotting, and deterministic scheduling inputs.
- Design notes:
  - Policy snapshots are hashed from ordered scope/rule lists and reused when the hash exists.
  - Torznab category handling preserves requested/effective lists and treats 8000 as catch-all.
  - Runnable indexers are filtered by profile allow/block rules, domain constraints, and policy allow_indexer_instance(require).
- Test coverage summary:
  - Not yet added; requires SQL stored-proc harness coverage for identifier parsing, category filtering, and runnable gating.
- Observability updates:
  - None in this change (DB-only procedure).
- Risk & rollback plan:
  - Risk: invalid gating logic could short-circuit legitimate searches. Rollback by reverting migration 0050 and re-running migrations.
- Dependency rationale:
  - No new dependencies added.
