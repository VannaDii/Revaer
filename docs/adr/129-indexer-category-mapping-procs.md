# Indexer category mapping procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Category mapping rules must be updated via stored procedures with validation and
    audit logging.
  - ERD_INDEXERS.md specifies media domain and Torznab category checks plus primary
    mapping enforcement.
- Decision:
  - Add migration 0039 implementing tracker_category_mapping and media_domain_to_torznab
    mapping upsert/delete procedures with stable wrappers.
  - Validate upstream_slug resolution, Torznab category IDs, and media domain keys.
  - Enforce a single primary mapping per media domain during upsert.
  - Record config_audit_log entries for all mutations.
- Consequences:
  - Category mapping changes are validated and auditable in the database.
  - Primary mapping invariants are enforced within the procedure transaction.
- Follow-up:
  - Add API handlers for category mapping management.
  - Add tests for primary switch and invalid key handling.
