# Indexer connectivity and reputation UI

- Status: Accepted
- Date: 2026-03-15
- Context:
  - `ERD_INDEXERS.md` requires operator-facing views for `indexer_connectivity_profile` and `source_reputation`, plus remediation-adjacent controls.
  - The derived tables and refresh jobs already existed, but the admin console could not inspect them without querying the database directly.
- Decision:
  - Add stored procedures and typed data/API/UI adapters to expose connectivity profile snapshots and recent reputation windows per indexer instance.
  - Reuse the existing instance admin surface and adjacent Cloudflare reset actions instead of creating a separate dashboard route first.
- Consequences:
  - Operators can now inspect connectivity status, dominant error class, latency, success rates, and recent reputation rollups from `/indexers`.
  - The implementation adds new read procedures and response DTOs that must stay aligned with derived-table schema changes.
- Follow-up:
  - Add richer health drill-down and notification delivery to close the remaining health dashboard checklist item.
  - Consider promoting these views into a dedicated health route if the admin console becomes too dense.
