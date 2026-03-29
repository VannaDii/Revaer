# Indexer instance test API and CLI

- Status: Accepted
- Date: 2026-01-31
- Context:
  - Indexer instance test stored procedures exist but lacked an API/CLI surface.
  - Executors need a prepare payload and a finalize endpoint to record outcomes.
- Decision:
  - Add API endpoints to prepare and finalize indexer instance tests.
  - Add CLI commands to invoke the prepare and finalize endpoints with JSON or table output.
  - Alternatives considered: defer until executor is built; rejected to keep parity with ERD flows.
- Consequences:
  - Positive outcomes: external executors and CLI can drive indexer test lifecycle.
  - Risks or trade-offs: test execution is still external; API must stay aligned with executor payload needs.
- Follow-up:
  - Wire executor to call the prepare/finalize API from the job runner.
  - Add E2E coverage for the test endpoints once executor is online.

## Task record

- Motivation: expose indexer instance test lifecycle via API/CLI to support migration and diagnostics.
- Design notes: API mirrors stored-proc inputs/outputs; CLI outputs field arrays and statuses.
- Test coverage summary: handler unit tests added for prepare/finalize; command label test updated.
- Observability updates: new service spans for prepare/finalize.
- Risk & rollback plan: revert API routes and CLI commands; no migrations or data changes.
- Dependency rationale: no new dependencies added.
