# Indexer core schema foundations

- Status: Accepted
- Date: 2026-01-25
- Context:
  - We need to begin implementing the indexer ERD with core, dependency-first tables.
  - The schema must follow ERD_INDEXERS.md and preserve SSOT for keys, IDs, and constraints.
- Decision:
  - Add a new migration that introduces the initial enum types and core tables:
    app_user, deployment_config, deployment_maintenance_state, trust_tier, media_domain, and tag.
  - Use bigint identity PKs, UUID public IDs, and explicit constraints per ERD.
- Consequences:
  - Positive: establishes the foundation required for indexer configuration and tagging.
  - Trade-off: further migrations are required to complete the full ERD.
- Follow-up:
  - Add remaining enum types and schema tables from ERD_INDEXERS.md.
  - Implement seed procedures and stored procedures for the new tables.

## Task record

- Motivation:
  - Start the indexer ERD implementation with the smallest dependency set.
- Design notes:
  - Enum types are defined for deployment_role, trust_tier_key, and media_domain_key.
  - Keys enforce lowercase checks; public UUIDs have no defaults to keep ownership in procedures.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if schema conflicts arise.
- Dependency rationale:
  - No new dependencies added.
