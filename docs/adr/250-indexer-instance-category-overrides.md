# Indexer instance category overrides

- Status: Accepted
- Date: 2026-03-20
- Context:
  - `ERD_INDEXERS.md` calls out custom category overrides as a parity gap versus Prowlarr, especially for cases where one indexer instance needs different tracker-to-Torznab mappings than the shared definition default.
  - The existing `tracker_category_mapping` storage and stored procedures only supported global mappings or definition-scoped mappings keyed by upstream slug.
  - The `/indexers` admin console did not expose any category override workflow, so operators could not safely persist or test instance-specific overrides.
- Decision:
  - Extend `tracker_category_mapping` with an optional `indexer_instance_id` scope and update the stored procedures to accept an optional `indexer_instance_public_id`.
  - When an instance scope is supplied, resolve its definition in-proc, reject deleted/missing instances, and reject conflicting definition-plus-instance combinations with a stable error code.
  - Add API model, handler, app-service, UI, and API/UI test coverage for instance-scoped tracker category mapping upsert and delete actions.
  - Alternative considered: a separate per-instance override table. That would have avoided a nullable column but would duplicate lookup logic and audit behavior that already belongs to the existing mapping entity.
- Consequences:
  - Operators can now tune category mappings for one indexer instance without changing the shared default for the definition.
  - The storage model is ready for later app-sync filtering work because mappings now have explicit instance scope in addition to global and definition scope.
  - App-scoped override behavior is still blocked on the separate app-sync UX/domain work, so the broader checklist item remains partially open until downstream app filtering is implemented.
- Follow-up:
  - Thread instance-scoped mappings into the downstream app-sync pipeline once app associations and sync profiles land.
  - Add app-specific override resolution rules when the app-sync domain slice is implemented.
