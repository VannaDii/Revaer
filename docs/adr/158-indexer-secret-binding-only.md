# Indexer secret binding linkage verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires secrets to be linked only through `secret_binding` and
    forbids inline `secret_id` columns on other tables.
  - We need to confirm the schema follows this rule before extending secret
    usage in routing and indexer configs.
- Decision:
  - Verified `secret` and `secret_binding` are the only tables owning
    `secret_id`, with bindings keyed by `(bound_table, bound_id, binding_name)`.
  - Confirmed other tables (e.g., `indexer_instance_field_value`,
    `routing_policy_parameter`) store no inline `secret_id` columns and rely on
    `secret_binding` for secret linkage.
- Consequences:
  - Secret linkage is centralized and auditable via `secret_binding` and
    `secret_audit_log`.
  - Schema aligns with ERD and avoids leaking secret references into unrelated
    tables.
- Follow-up:
  - Re-verify any new tables that require secret access ensure bindings are used.

## Task record

- Motivation:
  - Validate secrets are linked only through `secret_binding`.
- Design notes:
  - Reviewed `0015_indexer_secrets.sql` for secret/secret_binding tables and
    constraints.
  - Searched migrations for `secret_id` to confirm no inline secret references
    outside the secret tables.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future tables add direct `secret_id` columns. Roll back by removing
    inline references and migrating to `secret_binding`.
- Dependency rationale:
  - No new dependencies added.
