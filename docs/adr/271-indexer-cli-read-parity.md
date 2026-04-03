# Indexer CLI read parity

- Status: Accepted
- Date: 2026-04-03
- Context:
  - The ERD follow-up checklist still had a CLI parity gap even after the API and UI operator inventory surfaces landed.
  - Operators could inspect live tags, secrets, search profiles, policies, routing, rate limits, Torznab instances, RSS state, and health/connectivity from the web UI, but the CLI still only covered import, policy mutations, Torznab mutations, and test probes.
  - The next efficient step was to reuse existing authenticated GET endpoints instead of adding new backend scope.
- Decision:
  - Add a new `revaer indexer read ...` command group that maps directly to the existing operator read/list APIs.
  - Cover list/read flows for tags, secrets, search profiles, policy sets, routing policies, routing-policy detail, rate-limit policies, indexer instances, Torznab instances, backup export, per-instance connectivity, reputation, health events, RSS status, and RSS seen items.
  - Keep the implementation dependency-light by sharing a single typed GET helper in the CLI command layer and adding table/json renderers for the existing API model responses.
- Consequences:
  - CLI operators can now inspect the same live indexer inventory data that the `/indexers` UI uses, which materially narrows the parity gap without introducing new server behavior.
  - The change is low risk because it reuses stable GET endpoints and existing API model types instead of inventing duplicate transport contracts.
  - The broader CLI parity item remains open because write flows for tags, secrets, routing policies, rate limits, search profiles, backup restore, RSS mutation, health notification hooks, and category mappings still need command coverage.
- Follow-up:
  - Add the remaining CLI CRUD commands for the indexer admin surfaces once the read/list workflow settles.
  - Fold category-mapping and restore flows into the CLI before marking the reopened parity checklist item complete.
