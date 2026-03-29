# Indexer API boundary public-id verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires API boundaries to accept only UUID public IDs or keys and
    never expose internal bigint identities.
  - We need to confirm the current indexer API surface and stored-procedure
    entry points follow this rule.
- Decision:
  - Verified indexer API DTOs and handlers accept only UUIDs or keys, never
    internal bigint IDs.
  - Confirmed API DTOs for tags use `Uuid` for `tag_public_id` and string keys
    (`TagCreateRequest`, `TagUpdateRequest`, `TagDeleteRequest`) and that the
    indexer facade methods take UUID actor identities plus UUID/tag key inputs.
  - Confirmed indexer stored-procedure wrappers (`deployment_init`, `tag_*`,
    `routing_policy_*`, `rate_limit_*`, `search_*`, `secret_*`) accept UUID
    public IDs and key strings exclusively.
- Consequences:
  - API and stored-procedure boundaries comply with the ERD, keeping internal
    bigint identities private to the database layer.
  - Client integrations can rely on UUIDs/keys without leaking internal IDs.
- Follow-up:
  - Re-verify new indexer endpoints and procedures before expanding the API.

## Task record

- Motivation:
  - Validate API/public boundaries adhere to ERD public-id exposure rules.
- Design notes:
  - Checked tag API DTOs in `revaer-api-models` and the indexer facade/handlers
    in `revaer-api` for UUID-only identifiers.
  - Reviewed wrapper procs in migration `0064_indexer_wrapper_procs.sql`.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future endpoints accidentally expose internal IDs. Roll back by
    reverting the API shape and re-validating with stored-proc interfaces.
- Dependency rationale:
  - No new dependencies added.
