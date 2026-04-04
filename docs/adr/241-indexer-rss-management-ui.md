# Indexer RSS Management UI

- Status: Accepted
- Date: 2026-03-15
- Context:
  - The ERD checklist still lacked operator-facing RSS management despite stored procedures already supporting subscription writes and RSS dedupe storage.
  - The existing `/indexers` admin console had no way to inspect subscription cadence, view recently seen RSS items, or manually seed dedupe state.
- Decision:
  - Add stored-proc-backed RSS management APIs for subscription status, recent seen-item listing, and manual mark-seen.
  - Extend the indexer admin console with an RSS management panel that can fetch subscription state, update cadence/enablement, inspect recent items, and insert manual seen markers.
- Consequences:
  - Operators can now manage RSS polling behavior and dedupe history without direct database access.
  - The implementation adds new API/DTO surface area and one migration, which increases maintenance cost but keeps runtime SQL inside stored procedures.
- Follow-up:
  - Validate the new RSS panel in `just ci` and `just ui-e2e`.
  - Continue with the remaining unchecked migration items, especially health dashboards and deployment acceptance work.
