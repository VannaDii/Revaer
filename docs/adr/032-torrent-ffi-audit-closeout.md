# Torrent FFI Audit Closeout

- Status: Accepted
- Date: 2025-12-23
- Context:
  - The torrent FFI audit identified drift between API/runtime/FFI/native behavior (metadata updates, seed limits, proxy handling, IPv6 mode) and missing CI coverage for native tests.
  - The engine must remain a thin wrapper around libtorrent; unsupported knobs must be rejected early, and native settings must be auditable.
- Decision:
  - Reject unsupported metadata and per-torrent seed limit updates at the API boundary.
  - Remove Rust-side seeding enforcement and rely on native session settings only.
  - Enforce libtorrent version checks at build time and fail when unsupported.
  - Add native settings inspection hooks and native integration tests for proxy auth, seed limits, and IPv6 listen behavior.
  - Run native integration tests in CI via a dedicated just recipe.
- Consequences:
  - Drift between API/runtime and native behavior is eliminated for the audited settings.
  - Native test coverage is required in CI; local runs need libtorrent and Docker availability.
- Follow-up:
  - Keep FFI layout assertions updated as bridge structs evolve.
  - Extend native inspection snapshots when new settings are added.

## Motivation
Ensure the torrent engine remains a thin libtorrent wrapper by removing Rust-only semantics, rejecting unsupported updates at the API boundary, and enforcing native test coverage to prevent drift.

## Design Notes
- Added a lightweight native settings snapshot to validate applied proxy credentials, seed limits, and listen interfaces in tests.
- Adjusted native tests to assert deterministic events and avoid reliance on external swarm progress.
- Removed deprecated strict-super-seeding fallback in favor of version-gated settings.
- Updated FFI layout assertions after adding proxy auth and IPv6 fields.

## Test Coverage Summary
- `just test-native` exercises native unit and integration tests, including new assertions for proxy auth, seed limits, and IPv6 listen mode.
- `just ci` (run before handoff) covers workspace lint/test/cov/audit/deny gates.

## Observability Updates
- No new metrics; native settings snapshots are internal to test-only inspection.

## Risk & Rollback Plan
- Risk: native settings snapshot could drift if settings are renamed upstream.
- Rollback: revert to previous audit state and remove snapshot methods if libtorrent versions diverge; CI will flag mismatches quickly.

## Dependency Rationale
- No new dependencies introduced.
