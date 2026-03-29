# Indexer policy set procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Policy sets and rule toggles must be managed via stored procedures with role
    authorization and audit logging.
  - ERD_INDEXERS.md requires scope-based authorization and sort-order reordering.
- Decision:
  - Add migration 0040 implementing policy_set create/update/enable/disable/reorder and
    policy_rule enable/disable/reorder procedures with stable wrappers.
  - Enforce scope-specific authorization, cardinality rules for enabled global/user sets,
    and profile link requirement on enable.
  - Record config_audit_log entries for all mutations.
- Consequences:
  - Policy set lifecycle operations are validated and auditable at the DB layer.
  - Policy rule toggling and ordering are centralized for consistent behavior.
- Follow-up:
  - Implement policy_rule_create_v1 with value set payload handling.
  - Add API handlers and tests for policy set and rule operations.
