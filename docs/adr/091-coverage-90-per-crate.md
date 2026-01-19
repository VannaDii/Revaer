# 091: Raise per-crate coverage gate to 90%

- Status: Accepted
- Date: 2026-01-17
- Context:
  - The workspace coverage gate previously enforced ≥80% line coverage overall, which masked low-coverage crates.
  - The requirement is now ≥90% coverage per crate, without test-only code in production modules.
  - The gate must remain Justfile-driven and avoid llvm-cov suppression flags.
- Decision:
  - Update `just cov` to run `cargo llvm-cov` per crate and enforce a ≥90% threshold via the Justfile loop.
  - Raise the documented coverage requirement in `AGENT.md` to 90% per crate.
  - Add focused unit tests to raise coverage in low-coverage crates (test-support, asset_sync, doc-indexer, CLI, API setup/docs, UI ANSI parsing, libtorrent types).
- Consequences:
  - Coverage checks now report per-crate deficits with precise percentages.
  - The stricter gate currently fails on multiple crates until additional tests are added.
  - More test investment is required for large modules (API handlers, config loader, fsops pipeline, app bootstrap).
- Follow-up:
  - Add tests to raise coverage for: `revaer-app`, `revaer-config`, `revaer-data`, `revaer-fsops`, `revaer-api`, `revaer-ui`, `revaer-torrent-libt`, `asset_sync`, and `revaer-test-support`.
  - Re-run `just cov`, then complete the full `just ci` and `just ui-e2e` gates.

## Motivation
- Ensure test coverage reflects real production risk by enforcing ≥90% per crate.

## Design notes
- Coverage is computed per crate by running `cargo llvm-cov --package` in a workspace member loop.
- Crates with zero executable lines are treated as 100% covered by `llvm-cov` for that package.

## Test coverage summary
- `just cov` run on 2026-01-17; coverage gate failed. Current per-crate results:
  - revaer-app: 70.71% (1922/2718)
  - revaer-test-support: 71.30% (246/345)
  - revaer-data: 72.75% (993/1365)
  - revaer-config: 75.65% (2775/3668)
  - revaer-fsops: 76.15% (1520/1996)
  - asset_sync: 79.16% (300/379)
  - revaer-ui: 83.82% (1911/2280)
  - revaer-api: 84.37% (7539/8936)
  - revaer-torrent-libt: 85.38% (2961/3468)
  - revaer-cli: 86.51% (2084/2409)
  - revaer-doc-indexer: 89.73% (655/730)
  - revaer-telemetry: 92.40% (729/789)
  - revaer-torrent-core: 94.34% (250/265)
  - revaer-api-models: 95.34% (553/580)
  - revaer-events: 96.40% (268/278)
  - revaer-runtime: 100.00% (0/0)

## Observability updates
- None.

## Risk & rollback plan
- Risk: CI remains blocked until per-crate coverage is lifted to 90%.
- Rollback: revert the `just cov` loop and reset the coverage threshold (not recommended unless blocking critical releases).

## Dependency rationale
- No new dependencies added.
