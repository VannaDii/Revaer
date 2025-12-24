# Asset sync test stability under parallel runs

- Status: Accepted
- Date: 2025-12-24
- Context:
  - `cargo llvm-cov` runs tests in parallel and surfaced a flaky `asset_sync` test.
  - The temp directory helper used timestamp-based names that could collide under parallel execution.
  - CI requires `just ci` (including coverage) to pass reliably without intermittent failures.
- Decision:
  - Replace the time-based temp directory naming with a process id + atomic counter.
  - Retry on `AlreadyExists` to ensure unique per-test directories without new dependencies.
- Consequences:
  - Asset sync tests are deterministic under parallel runners and coverage instrumentation.
  - No new crates or runtime behavior changes.
- Follow-up:
  - None.

## Motivation

- Remove flaky coverage failures caused by temporary directory collisions in `asset_sync` tests.

## Design notes

- Use a static `AtomicUsize` counter plus `std::process::id()` to generate unique temp roots.
- Loop on `AlreadyExists` without introducing external dependencies.

## Test coverage summary

- `just ci` (fmt, lint, udeps, audit, deny, ui-build, test, cov).

## Observability updates

- None.

## Risk & rollback plan

- Risk: low; change is test-only.
- Rollback: revert the temp directory helper to its previous implementation.

## Dependency rationale

- No new dependencies.
