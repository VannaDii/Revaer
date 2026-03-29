# Indexer single-tenant scope verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD specifies a single-tenant deployment with no tenant scoping tables
    or tenant_id columns.
  - We need to confirm the schema has no tenant scoping artifacts.
- Decision:
  - Verified indexer migrations contain no tenant/organization scoping columns
    or tables.
  - Confirmed global catalog tables (e.g., `trust_tier`, `media_domain`,
    `indexer_definition`) are deployment-wide without tenant keys.
- Consequences:
  - Database schema aligns with the ERD's single-tenant scope assumptions.
  - Application layers can treat configuration and catalog data as global.
- Follow-up:
  - Re-verify if multi-tenant support is introduced in later phases.

## Task record

- Motivation:
  - Validate that the indexer schema remains single-tenant as required.
- Design notes:
  - Searched migrations for tenant/organization identifiers and found none.
  - Verified catalog tables are global with no scoping columns.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: accidental tenant columns creep into schema. Roll back by removing
    tenant fields and updating stored procedures.
- Dependency rationale:
  - No new dependencies added.
