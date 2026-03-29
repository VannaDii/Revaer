# Indexer secret schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD requires secret storage and auditable bindings for indexer field values and
    routing policy parameters.
  - Secret linkage must be centralized via secret_binding with revocation/rotation
    metadata.
- Decision:
  - Add secret, secret_binding, and secret_audit_log tables plus supporting enums.
  - Enforce binding_name allowlists per bound_table and key_id length checks.
- Consequences:
  - Positive: schema supports secure secret storage with auditable bindings.
  - Trade-off: follow-on migrations and procedures are required for lifecycle actions.
- Follow-up:
  - Implement secret procedures and auditing per ERD.
  - Add binding validation in indexer/routing procedures.

## Task record

- Motivation:
  - Continue ERD implementation with secrets storage and binding schema.
- Design notes:
  - secret_binding remains the only linkage, enforced by a bound_table/binding_name check.
  - secret_audit_log is append-only to capture create/rotate/revoke/bind/unbind actions.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
