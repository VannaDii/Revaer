# CI compliance cleanup for test error handling

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: restore `just ci` compliance and remove explicit panic/unwrap patterns in tests to align with AGENT error-handling rules.
  - Constraints: keep coverage ≥ 80% and avoid new dependencies while satisfying clippy::pedantic.
- Decision:
  - Replace explicit `panic!`/`unwrap` usages in tests with Result-returning flows and `let...else` patterns.
  - Exercise must-use values in tests to avoid lint violations.
- Consequences:
  - Positive outcomes: lint clean, tests remain deterministic, and coverage stays above the gate.
  - Risks or trade-offs: slightly more verbose test code; added Result plumbing in tests.
- Follow-up:
  - Implementation tasks: keep new tests using `Result` and `let...else` patterns when adding coverage.
  - Review checkpoints: re-run `just ci` after any test refactors.

## Design notes
- Tests now surface unexpected success paths as explicit error returns instead of panics.
- `Sse` test responses are exercised via `into_response` to satisfy must-use lints.

## Test coverage summary
- `just ci` completed with line coverage at 80.04%.

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: minimal; changes are confined to tests.
- Rollback: revert this ADR and the test-only edits, then re-run `just ci`.
