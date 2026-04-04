# Indexer import jobs API surface

- Status: Accepted
- Date: 2026-01-31
- Context:
  - Need REST coverage for indexer import jobs (create/run/status/results) to satisfy ERD indexer checklist.
  - Must preserve stored-procedure boundaries, stable errors, and testable handlers with E2E coverage.
- Decision:
  - Add import job request/response models and handler wiring for create/run/status/results endpoints.
  - Extend app facade mapping for import job error translation and results/status projection.
  - Update OpenAPI and Playwright API coverage for new endpoints.
  - Alternatives considered: defer API surface until full import pipeline; rejected to keep parity with checklist and procs.
- Consequences:
  - Positive outcomes: import job endpoints are now reachable, documented, and covered in E2E.
  - Risks or trade-offs: run endpoints currently validate inputs and return errors without a worker path; full import pipeline still pending.
- Follow-up:
  - Implement background import execution and UI flows for import job monitoring.
  - Extend CLI support once import pipeline is ready.

## Task record

- Motivation: close the ERD indexer checklist gap for import job REST endpoints and E2E coverage.
- Design notes: handlers trim inputs, map stored-procedure error codes to stable API errors, and return typed models; no inline SQL added.
- Test coverage summary: added API E2E coverage for create/run/status/results; existing unit tests cover trimming and error mapping.
- Observability updates: no new spans or metrics required for handler-only changes.
- Risk & rollback plan: rollback by reverting endpoint wiring and OpenAPI updates; no migrations or data changes.
- Dependency rationale: no new dependencies added; reused existing models, handlers, and stored procedures.
