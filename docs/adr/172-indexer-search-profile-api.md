# 172 Indexer search profile API endpoints

- Status: Accepted
- Date: 2026-01-31
- Context:
  - API coverage for search profile stored procedures was missing, blocking ERD checklist parity.
  - Prowlarr parity requires deterministic, auditable search profile configuration surfaces.
- Decision:
  - Add request/response models, facade methods, and HTTP routes for search profile lifecycle ops.
  - Keep error messages constant and attach context via structured fields.
- Consequences:
  - Search profiles can now be created and configured through the API layer.
  - E2E coverage asserts API availability for both auth modes.
- Follow-up:
  - Implement search profile UI surfaces and policy management endpoints.
  - Extend coverage for policy set integration once endpoints exist.

## Task record

- Motivation:
  - Expose stored-procedure-backed search profile management through the API.
  - Provide E2E coverage for search profile lifecycle operations to align with the ERD.
- Design notes:
  - Add API models for search profile create/update/default/domain allowlist/policy set/indexer allow-block/tag allow-block-prefer.
  - Extend the indexer facade to surface search profile operations with typed errors.
  - Implement HTTP handlers with constant error messages and trimmed inputs.
- Test coverage summary:
  - Unit tests for handler trimming and conflict mapping.
  - API E2E coverage for search profile lifecycle endpoints.
- Observability updates:
  - Reused existing tracing spans for indexer operations; no new metrics added.
- Risk & rollback plan:
  - Risk: invalid profile updates could affect search filtering.
  - Rollback: revert API changes and repair profiles via stored procedures/migrations.
- Dependency rationale:
  - No new dependencies.
