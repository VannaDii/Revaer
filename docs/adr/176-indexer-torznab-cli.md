# Indexer Torznab CLI management

- Status: Accepted
- Date: 2026-01-31
- Context:
  - Torznab instance keys and lifecycle operations are available via API but missing CLI tooling.
  - Need operator-level access to create, rotate, enable/disable, and delete Torznab instances.
- Decision:
  - Add `indexer torznab` CLI subcommands for create, rotate, set-state, and delete.
  - Render Torznab instance credentials in JSON or table output.
  - Alternatives considered: postpone CLI tooling; rejected to keep operational parity with API.
- Consequences:
  - Positive outcomes: CLI can manage Torznab instances and rotate keys without UI.
  - Risks or trade-offs: plaintext API keys are shown in CLI output; operators must handle securely.
- Follow-up:
  - Add CLI coverage once Torznab endpoints and auth rules are fully implemented.
  - Extend CLI for Torznab downloads and search flows when endpoints land.

## Task record

- Motivation: provide CLI access to Torznab instance creation, rotation, and state updates.
- Design notes: subcommands map 1:1 with REST endpoints and share existing output formatting patterns.
- Test coverage summary: command label tests updated; no new integration tests added.
- Observability updates: no additional telemetry beyond existing CLI emitter.
- Risk & rollback plan: revert CLI commands and output helpers; no migrations or data changes.
- Dependency rationale: no new dependencies added.
