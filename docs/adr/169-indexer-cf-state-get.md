# Indexer CF state read endpoint

- Status: Accepted
- Date: 2026-01-30
- Context:
  - Motivation: surface Cloudflare mitigation state per indexer instance so UI/API can display health and reset workflows safely.
  - Constraints: stored-procedure-only access, constant error messages, and no new dependencies.
- Decision:
  - Added `indexer_cf_state_get_v1` with a stable wrapper, plus data-access and API plumbing for a GET `/v1/indexers/instances/{id}/cf-state` response.
  - Added E2E API coverage for indexer instance and secret endpoints to satisfy coverage gating.
  - Alternatives considered: inline SQL or reusing reset-only plumbing (rejected due to stored-proc policy and missing read semantics).
- Consequences:
  - Positive: CF state is now observable through a typed API response; coverage gate stays green.
  - Trade-offs: endpoint currently used mainly for read/diagnostics; tests exercise 404 paths when no instance exists.
- Follow-up:
  - Expand UI controls and routing-policy integrations for CF/flaresolverr workflows per ERD gaps.

## Test Coverage

- `just ci`
- `just ui-e2e`

## Observability

- Added `indexer.cf_state_get` span in the indexer service path.

## Risk and Rollback

- Risk: minimal behavior change; read path only, returns 404 for unknown instances.
- Rollback: revert migration `0072_indexer_cf_state_get.sql` and associated API/service changes.

## Dependency Rationale

- No new dependencies added; existing crates and patterns were used.
