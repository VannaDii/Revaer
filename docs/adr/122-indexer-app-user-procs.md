# Indexer app_user stored procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - We need versioned, auditable entry points for app_user creation and maintenance.
  - ERD_INDEXERS.md requires normalized email storage, constant error messages, and wrapper
    procedures without version suffixes.
  - app_user has no audit fields, so procedures must be minimal and safe while preserving
    table invariants.
- Decision:
  - Add migration 0033 with app_user_create_v1, app_user_update_v1, and
    app_user_verify_email_v1 plus stable wrappers.
  - Normalize emails in-proc (trim + lowercase), enforce non-empty inputs, and default role
    to user with is_email_verified=false at creation.
  - Use constant error messages with detail codes for invalid or missing inputs.
- Consequences:
  - app_user mutations now go through stored procedures with consistent validation.
  - Email duplicates are rejected deterministically before insert.
  - Additional procedure surface requires maintenance when app_user rules evolve.
- Follow-up:
  - Update ERD_INDEXERS_CHECKLIST.md to mark app_user procedures complete.
  - Extend coverage when app_user endpoints are implemented.
