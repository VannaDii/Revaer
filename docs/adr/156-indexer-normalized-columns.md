# Indexer normalized column verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires normalized columns (e.g., `email_normalized`, `*_norm`)
    to support consistent lookups and lowercased comparisons.
  - We need to confirm the schema includes the specified normalized fields.
- Decision:
  - Verified `app_user.email_normalized` is present and enforced with a
    lowercase/trim CHECK constraint.
  - Verified generated normalized columns exist where specified in definition
    metadata (`indexer_definition_field_validation.text_value_norm` and
    `depends_on_value_plain_norm`).
  - Verified normalized identifier storage in search requests via
    `search_request_identifier.id_value_normalized`.
- Consequences:
  - Normalized fields are persisted in the schema for reliable matching and
    validation logic.
  - Stored procedures can rely on normalized columns without ad-hoc transforms.
- Follow-up:
  - Ensure any new ERD-defined normalized fields are added with the same
    constraints.

## Task record

- Motivation:
  - Confirm normalized columns exist for ERD-specified fields.
- Design notes:
  - Reviewed `0012_indexer_core.sql` for `email_normalized`.
  - Reviewed `0013_indexer_definitions.sql` for generated `*_norm` columns.
  - Reviewed `0023_indexer_search_requests.sql` for `id_value_normalized`.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: missing normalized columns can break lookup consistency. Roll back by
    adding the columns in migrations and updating procedures.
- Dependency rationale:
  - No new dependencies added.
