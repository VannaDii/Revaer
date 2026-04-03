# Indexer CLI operator write parity

- Status: Accepted
- Date: 2026-04-03
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had a reopened CLI parity gap after the read/list slice landed.
  - Operators could inspect indexer resources from the CLI, but tag lifecycle, secret lifecycle, and category-mapping writes still required the UI or raw API calls.
  - The next efficient step needed to reuse the existing stored-proc-backed HTTP surface instead of adding new runtime behavior.
- Decision:
  - Extend `revaer-cli` with `indexer tag`, `indexer secret`, and `indexer category-mapping` subcommands that call the existing `/v1/indexers/...` endpoints.
  - Keep the scope focused on operator write parity for tags, secrets, tracker category mappings, and media-domain mappings, with targeted CLI integration tests that assert exact request paths and payloads.
  - Leave the broader CLI parity checklist item open until routing-policy, rate-limit, search-profile, backup/restore, and RSS mutation flows also exist.
- Consequences:
  - Operators can now manage common indexer metadata and mapping writes from the CLI without dropping to raw HTTP.
  - The implementation stays dependency-light by reusing the existing reqwest client and output layer.
  - CLI parity is still incomplete overall, so the checklist must continue to call out the remaining mutation surfaces explicitly.
- Follow-up:
  - Add CLI write coverage for routing policies, rate limits, search profiles, and backup/restore.
  - Add CLI mutation flows for RSS state and any remaining category/profile assignment surfaces needed for full ERD parity.
