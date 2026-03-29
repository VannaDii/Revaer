# Indexer policy CLI management

- Status: Accepted
- Date: 2026-01-31
- Context:
  - Policy set and rule endpoints exist but lack CLI coverage.
  - Operators need a CLI path to create, enable, disable, and reorder policy sets and rules.
- Decision:
  - Add `indexer policy` CLI subcommands for policy set creation, update, enable/disable, reorder, and policy rule create/enable/disable/reorder.
  - Render policy set and rule identifiers in table or JSON output.
  - Alternatives considered: rely on API or UI; rejected to keep operational parity.
- Consequences:
  - Positive outcomes: CLI can manage policy sets and rules without UI.
  - Risks or trade-offs: CLI must be kept in sync with API schema updates.
- Follow-up:
  - Add list and detail commands once policy listing endpoints are available.
  - Expand rule creation ergonomics as policy rule value-set options grow.

## Task record

- Motivation: provide CLI access for policy sets and rules to match API capabilities.
- Design notes: subcommands mirror REST endpoints; requests validate non-empty fields locally.
- Test coverage summary: command label test updated; no new integration tests added.
- Observability updates: no additional telemetry beyond existing CLI emitter.
- Risk & rollback plan: revert CLI commands and output helpers; no migrations or data changes.
- Dependency rationale: no new dependencies added.
