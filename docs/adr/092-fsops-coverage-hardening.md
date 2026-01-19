# 092: Fsops coverage hardening

- Status: Accepted
- Date: 2026-01-17
- Context:
  - The workspace requires at least 90% per-crate line coverage (ADR 091).
  - `revaer-fsops` contained untested branches in pipeline helpers and filesystem routines.
- Decision:
  - Add targeted unit tests for fsops pipeline steps, rule parsing, and file operations.
  - Keep all test-only logic inside `#[cfg(test)]` modules.
  - Alternatives considered: integration tests backed by `RuntimeStore` + database; rejected for higher cost and slower feedback.
- Consequences:
  - Positive outcomes:
    - Improved coverage and regression protection for fsops edge cases.
  - Risks or trade-offs:
    - Additional filesystem IO during tests; mitigate with temp dirs and deterministic fixtures.
- Follow-up:
  - Run `just cov` and `just ci` to confirm the per-crate gate.
  - Watch for platform-specific permission semantics in CI.

## Motivation

Raise `revaer-fsops` coverage to meet the 90% per-crate gate while strengthening confidence in filesystem post-processing edge cases.

## Design notes

- Exercise both happy-path and skip/error branches without introducing production-only hooks.
- Favor direct unit tests of helper functions to keep the tests fast and deterministic.

## Test coverage summary

- Added unit tests for meta initialization, allowlist enforcement, glob parsing errors, archive extension checks, step short-circuiting, and file operation paths.
- Added permission/ownership tests for unix targets to cover `apply_permissions`, `resolve_owner`, and `resolve_group`.

## Observability updates

None; no runtime behavior changes.

## Risk & rollback plan

- Risk: file-permission tests may behave differently on non-unix systems.
- Rollback: revert the added tests and rework with platform guards if CI shows instability.

## Dependency rationale

No new dependencies added. Alternative considered: integration coverage via database-backed runtime store, rejected due to setup overhead.
