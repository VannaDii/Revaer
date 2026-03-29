# Indexer search profile procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Search profile mutations require scope-aware authorization, media domain resolution,
    and audit logging.
  - ERD_INDEXERS.md specifies default handling, allowlist semantics, and policy-set linkage.
- Decision:
  - Add migration 0041 implementing search_profile create/update/default operations plus
    domain allowlist, policy-set linking, indexer allow/block, and tag allow/block/prefer
    procedures with stable wrappers.
  - Enforce per-scope authorization, media domain key validation, and allow/block conflict
    checks.
  - Record config_audit_log entries for profile and rule updates.
- Consequences:
  - Search profile state changes are validated and auditable at the DB layer.
  - Allowlist and preference rules are kept consistent with block/allow constraints.
- Follow-up:
  - Implement API handlers for search profile management.
  - Add tests for default scope switching and allow/block conflicts.
