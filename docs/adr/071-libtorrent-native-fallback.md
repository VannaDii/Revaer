# 071: Libtorrent Native Fallback for Default CI

- Status: Accepted
- Date: 2026-01-02
- Context:
  - `just ci` runs `cargo udeps` across the workspace and fails on hosts without libtorrent headers or pkg-config data.
  - Native libtorrent integration tests are explicitly gated by `REVAER_NATIVE_IT`, so default runs should remain deterministic without requiring native system deps.
- Decision:
  - Gate native FFI compilation behind a build-time cfg (`libtorrent_native`) that is emitted only when libtorrent is discovered by `build.rs`.
  - When `REVAER_NATIVE_IT` is set, missing libtorrent is treated as an error; otherwise the build falls back to the stub backend with a warning.
  - Alternatives considered: require libtorrent for all CI/dev runs, or remove `--all-features` from the quality gates (rejected to keep feature coverage intact).
- Consequences:
  - Default `just ci` succeeds on machines without libtorrent while still honoring native coverage when explicitly requested.
  - Feature-enabled builds no longer guarantee native bindings unless libtorrent is present; native builds must opt in via `REVAER_NATIVE_IT`.
  - `cargo-udeps` ignores the `cxx` dependency for this crate because usage is gated by the native cfg.
- Follow-up:
  - Ensure native CI matrix jobs set `REVAER_NATIVE_IT=1` and install or bundle libtorrent.
