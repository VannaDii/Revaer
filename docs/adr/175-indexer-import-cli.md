# Indexer import jobs CLI commands

- Status: Accepted
- Date: 2026-01-31
- Context:
  - Import job API endpoints exist but CLI lacked parity for creating and inspecting import jobs.
  - Need to keep CLI output stable (json/table) and enforce API key requirements.
- Decision:
  - Add `indexer import` CLI subcommands for create, run (Prowlarr API/backup), status, and results.
  - Provide table and JSON output renderers for import job status and results.
  - Alternatives considered: postpone CLI until full import pipeline; rejected to close ERD checklist gap.
- Consequences:
  - Positive outcomes: operators can start and inspect import jobs from CLI with consistent output.
  - Risks or trade-offs: CLI surfaces are limited to import job endpoints; broader indexer CLI features remain pending.
- Follow-up:
  - Extend CLI with indexer test, policy management, and Torznab key commands.
  - Add CLI coverage once indexer workflows expand.

## Task record

- Motivation: provide CLI parity for indexer import job lifecycle operations.
- Design notes: new subcommands map 1:1 with REST endpoints and reuse common output formats.
- Test coverage summary: existing CLI unit tests extended for command label coverage; CLI integration not yet expanded.
- Observability updates: no new telemetry or metrics beyond existing CLI emitter.
- Risk & rollback plan: revert CLI subcommands and output helpers; no data changes.
- Dependency rationale: no new dependencies added; reused existing models and CLI utilities.
