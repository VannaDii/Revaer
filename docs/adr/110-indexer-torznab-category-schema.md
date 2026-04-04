# Indexer Torznab category schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD requires seeded Torznab categories and mappings to media domains and tracker
    categories for filtering and Torznab responses.
- Decision:
  - Add torznab_category, media_domain_to_torznab_category, and tracker_category_mapping
    tables with ERD constraints and uniqueness rules.
  - Enforce global uniqueness for tracker_category_mapping across null indexer_definition_id
    via a coalesced unique index.
- Consequences:
  - Positive: schema supports Torznab category lookups and tracker mapping overrides.
  - Trade-off: seeding and procedures remain follow-up work.
- Follow-up:
  - Seed Torznab categories and domain mappings per ERD.
  - Implement category mapping stored procedures and indexes.

## Task record

- Motivation:
  - Continue ERD implementation with Torznab category and mapping persistence.
- Design notes:
  - tracker_category and tracker_subcategory enforce non-negative values as specified.
  - media_domain mapping allows NULL media_domain_id for unsupported categories.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
