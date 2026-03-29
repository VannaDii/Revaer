# Indexer rate limit stored procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Rate limiting requires auditable policy management and a database-backed token bucket.
  - ERD_INDEXERS.md mandates bounds enforcement, system policy immutability, and scoped
    token consumption with minute windows.
- Decision:
  - Add migration 0037 implementing rate_limit_policy CRUD, instance/policy mappings,
    and rate_limit_try_consume_v1 plus stable wrappers.
  - Enforce owner/admin authorization, range checks, and in-use protection on delete.
  - Implement token bucket updates with row-level locking on rate_limit_state.
- Consequences:
  - Rate limit policies and assignments are centralized and auditable.
  - Token consumption is safe under concurrent access.
- Follow-up:
  - Integrate rate_limit_try_consume_v1 into outbound request logging.
  - Add tests for policy deletion conflicts and token bucket edge cases.
