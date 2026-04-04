# Indexer instance stored procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Indexer instances, RSS scheduling, domain/tag assignment, and field value management
    require validated, auditable mutations at the database layer.
  - ERD_INDEXERS.md mandates per-proc authorization, field validation, and audit logging.
- Decision:
  - Add migration 0038 implementing indexer_instance and RSS procedures, plus media domain,
    tag, and field value/secret binding procedures with stable wrappers.
  - Enforce owner/admin authorization, definition validation, and strict value checks
    (type, range, regex, allowed values).
  - Record config_audit_log updates for each mutation and secret_audit_log bind entries.
- Consequences:
  - Indexer configuration changes are validated and auditable in stored procedures.
  - Field validations are enforced consistently against definition rules.
- Follow-up:
  - Implement indexer_instance_test_v1 and outbound request logging integration.
  - Add API handlers and tests for indexer instance management.
