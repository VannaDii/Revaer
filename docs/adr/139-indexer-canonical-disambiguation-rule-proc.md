# Indexer canonical disambiguation rule procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Prevent-merge rules must enforce canonical identity normalization and symmetric uniqueness.
  - Only admin/owner users may create disambiguation rules.
- Decision:
  - Add `canonical_disambiguation_rule_create_v1` with normalization, identity validation, and canonical ordering of left/right pairs.
  - Record creation in `config_audit_log` with a canonical entity type.
- Consequences:
  - Duplicate or reversed rule pairs are rejected before insertion.
  - Invalid identity values fail fast and do not pollute rule sets.
- Follow-up:
  - Implement canonical merge/recompute procedures that honor prevent-merge rules.
