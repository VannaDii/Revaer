# Indexer system sentinel usage verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires system actions to use a sentinel user identifier
    (`user_id = 0` or the all-zero UUID) instead of NULL.
  - We need to confirm the indexer schema and stored procedures follow this
    rule before expanding automation workflows.
- Decision:
  - Verified the system sentinel user is seeded with `user_id = 0` and the
    all-zero UUID public ID in deployment seed and initialization migrations.
  - Confirmed stored procedures fall back to `user_id = 0` for system-driven
    actions (e.g., search request creation).
  - Confirmed data-layer tests use the all-zero UUID sentinel when invoking
    indexer procedures.
- Consequences:
  - System actions can be recorded without NULL audit fields, aligning with the
    ERD audit requirements.
  - Downstream API and UI layers can safely represent system activity with the
    sentinel UUID.
- Follow-up:
  - Re-verify new procedures or automation jobs continue to use the sentinel
    user IDs instead of NULL.

## Task record

- Motivation:
  - Validate that system actions always carry the sentinel user identifier.
- Design notes:
  - Seed/init migrations `0030_indexer_seed_data.sql`, `0032_indexer_deployment_init.sql`,
    and `0067_factory_reset_seed_defaults.sql` insert `user_id = 0` with the
    all-zero UUID.
  - `search_request_create_v1` defaults to `system_user_id := 0` when the actor
    is absent.
  - Data access tests (e.g., `crates/revaer-data/src/indexers/deployment.rs`)
    exercise stored procedures with the sentinel UUID.
- Test coverage summary:
  - Documentation-only verification; existing tests cover system-user usage.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: new procs may accept NULL actors. Roll back by enforcing sentinel
    defaults and updating callers/tests.
- Dependency rationale:
  - No new dependencies added.
