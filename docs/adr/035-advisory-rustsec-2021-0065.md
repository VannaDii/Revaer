# Advisory RUSTSEC-2021-0065 Temporary Ignore

- Status: Superseded by 073 (vendored yewdux exception tracked in ADR 074)
- Date: 2025-12-24
- Context:
  - The UI depends on `yewdux`, which transitively pulls `anymap` and triggers advisory `RUSTSEC-2021-0065` (unmaintained).
  - There is no maintained replacement for `anymap` within the pinned `yewdux` 0.9.x line, and upgrading yewdux would require a Yew major upgrade.
  - `cargo-audit` is configured to deny warnings, so ignoring the advisory requires explicit documentation and a remediation plan.
- Decision:
  - Add `RUSTSEC-2021-0065` to `.secignore` while `yewdux` requires `anymap`.
  - Track `yewdux` upgrades or alternatives that remove `anymap` and remove the ignore when available.
  - No runtime mitigation is required beyond limiting use to the UI state store.
- Consequences:
  - CI remains green while upstream resolves the dependency.
  - The unmaintained dependency remains in the tree until we migrate away from it.
- Follow-up:
  - Re-evaluate `yewdux` upgrade paths quarterly; remove the ignore once `anymap` is no longer required.
  - If upstream is stalled, evaluate a UI store replacement or a fork that removes `anymap`.
- Superseded: `.secignore` cleaned in ADR 073; vendored yewdux exception tracked in ADR 074 (no `anymap` crate dependency reintroduced).

## Motivation

- Keep `just audit` passing without blocking UI state work while documenting the risk and path to remediation.

## Design notes

- The ignore is scoped to the single advisory and is documented in `.secignore` with this ADR for traceability.

## Test coverage summary

- `just ci` (includes fmt, clippy, udeps, audit, deny, test, cov).

## Observability updates

- None; advisory handling does not change runtime telemetry.

## Risk & rollback plan

- Risk: unmaintained dependency stays in the build; monitor upstream advisories and plan a migration.
- Rollback: remove `yewdux` usage and replace with a small local store implementation or upgrade to a supported release once available.

## Dependency rationale

- `yewdux` provides the shared store needed for the UI; alternatives considered were a custom store (higher lift) or upgrading to `yewdux` 0.11+ (requires Yew 0.21+ migration).
