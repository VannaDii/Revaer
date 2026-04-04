# Indexer CLI health-notification parity

- Status: Accepted
- Date: 2026-04-03
- Context:
  - The reopened `ERD_INDEXERS_CHECKLIST.md` CLI parity item was down to one operator workflow gap after the read/list and broader mutation slices landed.
  - Health notification hooks already existed in the stored-proc-backed API and UI, but operators still could not manage them from `revaer-cli`.
  - Leaving that one workflow behind would keep the broader CLI parity item artificially open even though the rest of the indexed management surface was already exposed.
- Decision:
  - Add `revaer indexer read health-notifications` plus `revaer indexer health-notification create|update|delete` command flows on top of the existing `/v1/indexers/health-notifications` API surface.
  - Reuse the current request helpers, trimmed-string validation, and table/json output conventions instead of adding new transport abstractions.
  - Add focused CLI integration-style tests for one read path and one mutation path to keep the new surface covered without duplicating API behavior tests.
- Consequences:
  - Operators can now inspect and manage indexer health notification hooks from the CLI with the same stored-proc-backed behavior already available over HTTP and in the UI.
  - The reopened CLI parity checklist item can now close, leaving the remaining ERD gaps concentrated in runtime executors and stronger live acceptance coverage.
  - No new dependencies were required; the slice stays within the existing CLI/request/output structure.
