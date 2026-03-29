# Secret Binding And Test Error Class Coverage

- Status: Accepted
- Date: 2026-03-08
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had the migration acceptance item for secret binding/test UX unchecked.
  - The stored procedures already implemented the intended behavior, but the repo lacked focused coverage proving successful secret binding, missing-secret test preparation failures, and success-path clearing of migration error state.
- Decision:
  - Add data-layer coverage for routing policy secret binding persistence.
  - Add executor coverage for missing required secret preparation failures, successful bound-secret preparation payloads, and finalize-success clearing of migration error state.
  - Add API coverage for routing-policy secret bind problem details preserving the stable `error_code` context, plus API E2E coverage for successful and revoked-secret binding flows.
- Consequences:
  - The migration acceptance item is now backed by direct stored-proc, handler, and API end-to-end tests instead of inference from adjacent behavior.
  - Coverage now proves the ERD-required `missing_secret` and secret lifecycle behavior without adding new dependencies or widening public APIs.
  - The remaining ERD work is still broader than this acceptance item; this ADR closes only the secret binding/test UX gap.
- Follow-up:
  - Keep extending instance-level public flows once definition-selection UX stops relying on internal IDs.
  - Revisit checklist items tied to broader API/public-surface cleanup separately.
