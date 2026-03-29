# Indexer external reference public-id verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires external references (policy rules, disambiguation rules)
    to store UUID public IDs or keys instead of internal bigint identities.
  - We need to confirm the schema matches that rule before expanding policy and
    canonicalization workflows.
- Decision:
  - Verified policy and disambiguation tables store UUIDs or keys only for
    external references.
  - Policy rules capture external identifiers via `match_value_uuid` or
    lowercase `match_value_text` and `policy_rule_value_set_item.value_uuid`.
  - Policy snapshots store `policy_rule_public_id` UUIDs.
  - Canonical disambiguation rules store UUIDs only when referencing
    `canonical_public_id`, otherwise text hashes.
- Consequences:
  - External reference data can be safely exposed in APIs without leaking
    internal bigint IDs.
  - Internal joins still rely on bigint PKs, preserving database integrity.
- Follow-up:
  - Re-verify future policy/disambiguation changes keep UUID/key-only
    references.

## Task record

- Motivation:
  - Validate that external references never store internal bigint IDs.
- Design notes:
  - Reviewed `policy_rule` and `policy_rule_value_set_item` columns in
    `0019_policy_sets.sql`.
  - Reviewed `canonical_disambiguation_rule` in `0022_indexer_canonicalization.sql`.
  - Reviewed `policy_snapshot_rule` usage of `policy_rule_public_id`.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future migrations introduce bigint references in external-facing
    columns. Roll back by reverting schema changes and updating procedures.
- Dependency rationale:
  - No new dependencies added.
