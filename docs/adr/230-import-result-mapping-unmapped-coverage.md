# 229: Import result mapping and unmapped-definition coverage

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD acceptance requires imported indexers to either map to definitions or surface an explicit unmapped state.
  - Existing import-job coverage did not assert the combined status/result behavior for mapped and unmapped outcomes.
- Decision:
  - Add data-layer stored-procedure coverage for import-job status aggregation and result listing with mixed mapped/unmapped outcomes.
  - Validate that:
    - mapped results are represented by `imported_ready` with `upstream_slug` set;
    - unmapped results are represented by `unmapped_definition` with `upstream_slug` unset and explicit detail.
- Consequences:
  - Positive outcomes:
    - Import status aggregation and result projection now enforce ERD-required unmapped explainability.
    - Regression risk for import result classification is reduced.
  - Risks or trade-offs:
    - Test setup inserts fixture rows directly into `import_indexer_result` to model importer output states.
- Follow-up:
  - Extend API/UI import flows to render unmapped result remediation actions when import UX work is implemented.

## Motivation
- Close a migration acceptance gap with executable checks for mapped vs unmapped import outcomes.

## Design notes
- Reused existing `import_job_create`, `import_job_get_status`, and `import_job_list_results` stored-proc wrappers.
- Added a single focused test that seeds two result rows and validates both rollup counters and list projections.

## Test coverage summary
- Added `import_job_status_and_results_surface_unmapped_definitions` in `crates/revaer-data/src/indexers/import_jobs.rs`.

## Observability updates
- No telemetry changes; this is behavior-verification coverage.

## Risk & rollback plan
- If result classification semantics change, update procedure definitions and this coverage together.
- Roll back by reverting this ADR and the associated test.

## Dependency rationale
- No new dependencies added.
