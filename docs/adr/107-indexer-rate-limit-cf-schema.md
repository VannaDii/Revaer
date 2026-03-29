# Indexer rate limit and Cloudflare schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD defines rate limiting policy/state and Cloudflare status tracking for indexers.
  - These tables are prerequisites for routing enforcement and job-based cleanup.
- Decision:
  - Add rate_limit_policy, indexer_instance_rate_limit, routing_policy_rate_limit,
    rate_limit_state, and indexer_cf_state tables plus required enums.
  - Enforce ERD ranges, uniqueness, and cascade delete for instance/routing children.
- Consequences:
  - Positive: schema supports rate limit configuration, token tracking, and CF state.
  - Trade-off: stored procedures and scheduled jobs remain follow-up work.
- Follow-up:
  - Implement rate_limit and cf_state procedures, including seed defaults and purge jobs.
  - Add outbound_request_log integration and derived connectivity profile.

## Task record

- Motivation:
  - Continue ERD implementation with rate limiting and CF state persistence.
- Design notes:
  - rate_limit_state uses per-minute buckets with non-negative token usage.
  - indexer_cf_state enforces non-negative backoff and consecutive failure counters.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
