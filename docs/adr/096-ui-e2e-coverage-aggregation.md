# Aggregate UI E2E coverage for sharded runs

- Status: Accepted
- Date: 2026-01-23
- Context:
  - What problem are we solving?
    - Playwright sharding runs global teardown per shard, causing partial coverage checks to fail.
  - What constraints or forces shape the decision?
    - Keep Playwright invoked via `just ui-e2e`, avoid new dependencies, and preserve coverage gating.
- Decision:
  - Summary of the choice made.
    - Skip coverage assertions in sharded teardown, write shard-specific coverage files, upload them as artifacts, and run an aggregate coverage check in a dedicated job.
  - Alternatives considered.
    - Disable coverage checks entirely for sharded runs (reduces signal).
    - Keep non-sharded UI E2E only (slower feedback).
- Consequences:
  - Positive outcomes.
    - Sharded UI E2E runs succeed while retaining full coverage enforcement.
  - Risks or trade-offs.
    - Additional workflow job and artifact handling.
- Follow-up:
  - Implementation tasks.
    - Monitor shard duration and artifact sizes.
  - Review checkpoints.
    - Revisit shard count if coverage aggregation becomes slow.

## Task record

- Motivation: Fix sharded UI E2E failures while maintaining coverage enforcement.
- Design notes: Shard-specific coverage files with an aggregate coverage check via `just ui-e2e-coverage`.
- Test coverage summary: `just ci`, `just ui-e2e`.
- Observability updates: None (workflow-only change).
- Risk & rollback plan: Revert sharding and coverage aggregation changes if instability persists.
- Dependency rationale: No new dependencies introduced.
