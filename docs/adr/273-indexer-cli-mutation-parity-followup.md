# Indexer CLI mutation parity follow-up

- Status: Accepted
- Date: 2026-04-03
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had the reopened CLI parity item open after the earlier read/list and tag/secret/category-mapping slices landed.
  - Operators still needed the UI or raw API calls for routing-policy writes, rate-limit management, search-profile mutation, backup restore, and RSS state mutation.
  - Those flows already existed behind stored-proc-backed HTTP endpoints, so the next efficient step was to expose them through `revaer-cli` instead of adding new backend behavior.
- Decision:
  - Extend `revaer-cli` with `indexer routing-policy`, `indexer rate-limit`, `indexer search-profile`, `indexer backup restore`, and `indexer rss` command groups that call the existing `/v1/indexers/...` endpoints.
  - Keep backup restore file-driven by reading the exported snapshot JSON and posting it as an `IndexerBackupRestoreRequest`.
  - Add focused CLI integration coverage for representative new mutation paths instead of duplicating every endpoint-level API test in the CLI crate.
- Consequences:
  - Operators can now manage the bulk of indexer mutation flows from the CLI without dropping to raw HTTP.
  - The implementation stays dependency-light by reusing the existing request helpers and output renderers.
  - The broader CLI parity checklist item still remains open because health-notification hook mutation parity has not landed yet.
- Follow-up:
  - Add CLI mutation flows for health-notification hooks to close the remaining reopened CLI parity gap.
  - After the CLI item is closed, focus the remaining reopened ERD work on live runtime execution and stronger acceptance coverage.
