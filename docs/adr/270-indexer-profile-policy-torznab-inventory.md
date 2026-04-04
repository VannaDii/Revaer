# Indexer profile, policy, and Torznab inventory

- Status: Accepted
- Date: 2026-04-03
- Context:
  - The branch-analysis follow-up reopened the operator read/list gap because the admin console still depended on pasted UUIDs for search profiles, policy sets/rules, and Torznab instances.
  - `ERD_INDEXERS.md` expects existing resources to be inspectable over API and UI, not only writable through CRUD endpoints.
- Decision:
  - Add stored-procedure-backed list reads for search profiles, policy sets with rules, and Torznab instances, then expose them through `/v1/indexers/search-profiles`, `/v1/indexers/policies`, and `/v1/indexers/torznab-instances`.
  - Reuse those inventories in `/indexers` so operators can prefill app-sync, policy, Torznab, and category-mapping actions from live data instead of remembered IDs.
- Consequences:
  - The remaining operator inventory gap is closed for the existing ERD-backed resource set: instances, routing policies, search profiles, policy sets/rules, Torznab instances, rate limits, tags, and secret metadata are all inspectable from API and UI.
  - The data layer now has additional stable proc surfaces that must stay aligned with the schema-catalog test and exported OpenAPI document.
- Follow-up:
  - Keep CLI parity work separate; this ADR only closes the API/UI inspection surface.
  - Preserve list payload stability because the admin console and API E2E specs now depend on them.
