# Advisory RUSTSEC-2025-0141 Temporary Ignore

- Status: In Progress
- Date: 2026-01-11
- Context:
  - `bincode` 1.3.3 is flagged as unmaintained (RUSTSEC-2025-0141).
  - The dependency is pulled via `gloo-worker` in `gloo`, which is required by the Yew UI stack.
  - No drop-in upgrade path is available without upstream releases.
- Decision:
  - Add `RUSTSEC-2025-0141` to `.secignore` while the UI depends on `gloo`/`yew` that transitively require `bincode` 1.3.3.
  - Revisit once upstream releases remove or replace the dependency.
- Consequences:
  - `just audit` passes while the advisory remains documented.
  - The unmaintained dependency stays in the tree until upstream updates land.
- Follow-up:
  - Track `gloo` and `yew` release notes for `bincode` replacement/removal.
  - Remove the ignore once the dependency graph no longer includes `bincode` 1.3.x.

## Motivation

- Keep `just ci` passing while capturing the risk and remediation plan for the unmaintained transitive dependency.

## Design notes

- The ignore is scoped to the single advisory and documented in `.secignore` plus this ADR.

## Test coverage summary

- `just ci` (includes fmt, clippy, udeps, audit, deny, test, cov).

## Observability updates

- None; advisory handling does not change runtime telemetry.

## Risk & rollback plan

- Risk: unmaintained dependency remains in the build while upstream updates are pending.
- Rollback: remove the ignore after upgrading `gloo`/`yew` or replacing the dependency.

## Dependency rationale

- `gloo` and `yew` are required for the UI; alternatives would require a larger frontend migration.
