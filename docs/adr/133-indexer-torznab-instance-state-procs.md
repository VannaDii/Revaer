# Indexer Torznab instance state procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Torznab instances need enable/disable and soft-delete operations with role-based authorization tied to their search profiles.
  - Stored procedures must enforce invariants and write audit logs.
- Decision:
  - Add `torznab_instance_enable_disable_v1` and `torznab_instance_soft_delete_v1` with search-profile scoped authorization and audit logging.
  - Keep create/rotate key procedures separate to accommodate pending secret-key hashing decisions.
- Consequences:
  - Torznab instances can be safely toggled or retired without exposing API key material.
  - Create/rotate remain blocked until API key hashing strategy is finalized.
- Follow-up:
  - Implement `torznab_instance_create_v1` and `torznab_instance_rotate_key_v1` once Argon2id hashing is approved for the database layer or moved to the app layer.
