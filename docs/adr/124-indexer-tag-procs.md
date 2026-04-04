# Indexer tag stored procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Tags are user-created and soft-deleted; procedures must preserve tag_key immutability.
  - ERD_INDEXERS.md requires audit logging, lowercase tag keys, and conflict handling when
    tag_public_id and tag_key are both provided.
  - Stored procedures need constant error messages with structured detail codes.
- Decision:
  - Add migration 0034 with tag_create_v1, tag_update_v1, and tag_soft_delete_v1 plus
    stable wrappers.
  - Validate tag_key casing, length, and uniqueness on create; tag_key is immutable on
    update and delete.
  - Support tag resolution by public ID and/or key with invalid_tag_reference on conflict.
  - Write config_audit_log entries for create, update, and soft-delete actions.
- Consequences:
  - Tag mutations are centralized and auditable in the database layer.
  - Additional procedure surface area must be kept in sync with future tag rules.
- Follow-up:
  - Extend REST handlers to use tag procedures with key/public ID resolution.
  - Add API validation tests for invalid_tag_reference and soft-delete behaviors.
