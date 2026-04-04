# Indexer seed data and defaults

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD_INDEXERS.md requires seeded trust tiers, media domains, Torznab categories,
    default rate limits, and job scheduling rows.
  - Seed functions must enforce immutability rules for system-owned data.
- Decision:
  - Add migration 0030_indexer_seed_data.sql with seed procedures and inserts.
  - Seed Torznab categories, media-domain mappings, tracker mappings, rate limits,
    job schedules, and the system user.
- Consequences:
  - Deployments start with required lookup data and system defaults.
  - Seeded values are validated for consistency on migration.
- Follow-up:
  - Implement deployment_init_v1 and remaining stored procedures.

## Task record

- Motivation:
  - Provide required seed data and seed procedures for indexer ERD compliance.
- Design notes:
  - trust_tier_seed_defaults and media_domain_seed_defaults are idempotent and
    validate seeded values.
  - job_schedule rows use randomized initial next_run_at jitter.
- Test coverage summary:
  - No new tests added; migrations validated via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting migration 0030 if seed values require adjustment.
- Dependency rationale:
  - No new dependencies added.
