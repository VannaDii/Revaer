# Indexer public-id and bigint identity verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD mandates bigint identity primary keys and UUID public IDs for specific
    indexer tables, while indexer_definition must not expose a public ID in v1.
  - API and service layers depend on stable public identifiers without leaking internal
    bigint keys.
- Decision:
  - Verify the following tables use `BIGINT GENERATED ALWAYS AS IDENTITY` primary keys and
    enforce UUID public IDs (unique) where required:
    - app_user
    - indexer_instance
    - routing_policy
    - policy_set
    - policy_rule
    - search_profile
    - search_request
    - canonical_torrent
    - canonical_torrent_source
    - torznab_instance
    - rate_limit_policy
    - secret
  - Confirm `indexer_definition` has no public ID in v1.
- Consequences:
  - Indexer APIs can safely use UUIDs/keys without exposing internal bigint IDs.
  - Table definitions align with ERD identity rules, reducing migration drift.
- Follow-up:
  - Re-verify new tables against this rule before adding API or UI surfaces.

## Task record

- Motivation:
  - Validate ERD identity/public ID rules before expanding indexer-facing APIs.
- Design notes:
  - Verified table definitions in migrations 0012, 0014, 0015, 0016, 0018, 0019, 0022, and
    0023 include bigint identity PKs and required public IDs.
  - Verified `indexer_definition` in 0013 contains no public ID column.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future migrations add missing or redundant public IDs. Rollback by reverting the
    offending migration and revalidating against the ERD.
- Dependency rationale:
  - No new dependencies added.
