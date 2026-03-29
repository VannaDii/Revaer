# Import Result Fidelity Snapshots

- Status: Accepted
- Date: 2026-03-07
- Context:
  - The ERD migration checklist requires imported indexers to preserve enabled state, categories, tags, priorities, and missing-secret detection.
  - The current import job surface only returned coarse result status, which made parity verification impossible even in dry-run and partial-import paths.
  - Runtime DB interactions must stay on stored procedures, persisted data must remain normalized, and no JSON/JSONB snapshots are allowed.
- Decision:
  - Extend `import_indexer_result` with scalar fidelity fields for `resolved_is_enabled`, `resolved_priority`, and `missing_secret_fields`.
  - Persist multi-value fidelity snapshots in normalized child tables: `import_indexer_result_media_domain` and `import_indexer_result_tag`.
  - Expand `import_job_list_results_v1` and the API/CLI DTO contract to return the preserved snapshot for each result.
  - Alternatives considered: storing arrays directly on `import_indexer_result`, which was rejected because it weakens normalization and makes future filtering harder; deferring all fidelity reporting until the full importer exists, which would leave the migration checklist untestable.
- Consequences:
  - Import result payloads now carry enough data to verify category/tag/priority/secret preservation rules.
  - The schema grows by two operational child tables and one proc contract expansion, which increases migration and test surface slightly.
  - This does not implement full Prowlarr ingestion by itself; it establishes the normalized persistence and observable contract the importer will write to.
- Follow-up:
  - Wire the actual Prowlarr API/backup importer to populate the new snapshot fields and child tables.
  - Add API/E2E coverage once an executable import path can create populated results through HTTP.
  - Review whether secret error-class detail should include field names or only counts once the importer is implemented.

## Task Record

Motivation: make the ERD migration-fidelity acceptance item measurable with the current import-job surface.

Design notes: scalar fidelity lives on `import_indexer_result`; category and tag snapshots stay normalized in dedicated child tables; the stored procedure returns sorted arrays for a stable API contract.

Test coverage summary: added data-layer integration coverage for preserved import result snapshots and updated schema catalog expectations for the new tables.

Observability updates: no new metrics were needed; existing import job spans and outcome counters remain the boundary for this read-path change.

Risk & rollback plan: if the contract causes downstream issues, revert migration `0097_import_result_fidelity_snapshot.sql` and the DTO mapping change together; the change is isolated to import-result persistence and listing.

Dependency rationale: no new dependencies were added; existing SQLx, serde, and chrono types already cover the new fields.
