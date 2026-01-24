# UI E2E sharding in workflows

- Status: Accepted
- Date: 2026-01-23
- Context:
  - What problem are we solving?
    - UI E2E runs are long and delay feedback, especially when other jobs have already passed.
  - What constraints or forces shape the decision?
    - Keep Playwright invoked through `just ui-e2e` and avoid new dependencies.
- Decision:
  - Summary of the choice made.
    - Add Playwright sharding support to `just ui-e2e` and shard the UI E2E jobs with a matrix in CI/PR workflows.
  - Alternatives considered.
    - Increase test workers only (limited benefit because suite already uses Playwright workers).
    - Split tests by directory into separate workflows (more maintenance).
- Consequences:
  - Positive outcomes.
    - Reduced wall-clock time for UI E2E runs via parallel shards.
  - Risks or trade-offs.
    - Increased parallel runner usage for sharded jobs.
- Follow-up:
  - Implementation tasks.
    - Monitor shard duration balance and tune shard counts if needed.
  - Review checkpoints.
    - Reassess sharding if runner usage limits become a concern.

## Task record

- Motivation: Parallelize UI E2E to shorten CI runtime while keeping the just-based workflow contract intact.
- Design notes: Use Playwright's `--shard` flag driven by `PLAYWRIGHT_SHARD_INDEX` and `PLAYWRIGHT_SHARD_TOTAL`.
- Test coverage summary: `just ci` and `just ui-e2e` passed.
- Observability updates: None (workflow-only change).
- Risk & rollback plan: Revert sharding env and matrix changes if shard stability or runner usage is problematic.
- Dependency rationale: No new dependencies introduced.
