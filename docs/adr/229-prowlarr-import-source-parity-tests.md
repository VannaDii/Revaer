# 228: Prowlarr import source parity and dry-run coverage

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD acceptance requires import jobs to support both `prowlarr_api` and `prowlarr_backup` sources with dry-run mode.
  - Existing coverage did not explicitly assert source-specific run-path behavior and dry-run persistence across both source modes.
- Decision:
  - Add `revaer-data` tests to validate:
    - `import_job_create` persists `prowlarr_backup` with `is_dry_run=true`.
    - `import_job_run_prowlarr_api` and `import_job_run_prowlarr_backup` reject mismatched job source with `import_source_mismatch`.
  - Extend API E2E import job coverage to execute both run paths against matching and mismatched sources.
- Consequences:
  - Positive outcomes:
    - Source parity and dry-run behavior are validated at both stored-proc and API boundary levels.
    - Regression risk for import source routing logic is reduced.
  - Risks or trade-offs:
    - Slightly longer API E2E runtime due to additional import job flows.
- Follow-up:
  - Add UI import wizard coverage when import UX lands, so dry-run and source selection are exercised from UI paths.

## Motivation
- Close a checklist gap with executable verification for ERD-required import source behavior.

## Design notes
- Reused existing integration harnesses; no production logic changes were required.
- Asserted database `DETAIL` codes to keep failure modes explicit and stable.

## Test coverage summary
- `crates/revaer-data/src/indexers/import_jobs.rs`:
  - `import_job_create_supports_backup_source_and_dry_run`
  - `import_job_run_procedures_reject_source_mismatch`
- `tests/specs/api/indexers-import-jobs.spec.ts`:
  - Added backup-source creation/run and cross-source mismatch assertions.

## Observability updates
- No new telemetry emitted; this change increases behavioral coverage only.

## Risk & rollback plan
- If these assertions conflict with intended semantics, update stored-proc details and tests in lockstep.
- Roll back by reverting this ADR and test updates.

## Dependency rationale
- No new dependencies added.
