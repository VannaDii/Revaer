# Indexer search profiles and Torznab schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD requires search profiles to capture user intent and Torznab instances to expose
    arr-compatible endpoints tied to profiles.
  - Import jobs depend on search_profile and torznab_instance references.
- Decision:
  - Add schema for search_profile and related allow/block/prefer tables plus torznab_instance.
  - Enforce ERD constraints for page sizing, weight ranges, and uniqueness.
- Consequences:
  - Positive: enables profile filtering and Torznab endpoint configuration in the schema.
  - Trade-off: policy_set linking and import pipeline remain follow-up migrations.
- Follow-up:
  - Add search_profile_policy_set once policy_set exists.
  - Implement import_job tables and Torznab procedures after policy/schema dependencies.

## Task record

- Motivation:
  - Continue ERD implementation with search profile and Torznab persistence.
- Design notes:
  - Weight overrides allow nullable values with bounded ranges per ERD notes.
  - torznab_instance stores hashed API keys only, with soft-delete support.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
