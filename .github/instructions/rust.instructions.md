---
applyTo:
  - "Cargo.toml"
  - "rust-toolchain.toml"
  - ".clippy.toml"
  - "deny.toml"
  - "justfile"
  - "crates/**/*.rs"
  - "crates/**/Cargo.toml"
  - "tests/**/*.rs"
  - "scripts/**/*.rs"
---

`AGENTS.md` is the root contract. This file only tightens or specializes Rust-related guidance.

# Rust Quality Rules

- Production and bootstrap Rust must be deterministic and panic-free.
- `panic!`, `unwrap()`, `expect()`, and `unreachable!()` are forbidden in authored production and bootstrap code.
- `todo!()` and `unimplemented!()` are forbidden in authored Rust. Split the work or delete the dead path instead of leaving stubs behind.
- Tests should prefer `Result`-returning flows and explicit assertions over `unwrap()` and `expect()`. Use panic-based helpers only when the behavior under test is itself a panic boundary.
- `Option<T>` is valid only for expected absence or partial-function semantics. Do not use it to hide I/O, validation, persistence, network, or parsing failure.
- `Result<T, E>` is required for recoverable failure, including `Result<(), E>` for side-effecting operations that can fail.
- `catch_unwind` is forbidden outside the FFI boundary shims covered by `ffi.instructions.md`.
- Silent suppression is forbidden. `let _ = fallible_operation();` is not acceptable unless the returned value is provably infallible and intentionally discarded.
- Errors are logged once at the origin point, then propagated as data.

# Lint And Cfg Hygiene

- Keep workspace lint posture aligned with `AGENTS.md`, the active `just` recipes, and crate-root attributes.
- `just lint` includes `scripts/policy-guardrails.sh`. Keep that guardrail aligned with the root policy when the lint posture changes.
- `scripts/policy-guardrails.sh` currently enforces no source-level lint suppressions, no authored stubs, FFI-only `unsafe`/`catch_unwind`, and the stored-procedure-only runtime SQL boundary. The inline DDL/DML scan is case-insensitive, excludes test-only sidecar modules at `crates/**/src/**/tests.rs`, and must keep working when `rg` is unavailable by falling back to the tracked Rust file list.
- `just lint` also runs a production-target Clippy pass on workspace libs, bins, and examples that forbids `panic!`, `unwrap()`, `expect()`, `unreachable!()`, `todo!()`, and `unimplemented!()` without applying those restrictions to test targets.
- Keep repo-level Clippy exceptions in `just lint`, not in crate source. Today that includes the ADR-backed `clippy::multiple_crate_versions` exception and the workspace `pub(crate)` style exception for `clippy::redundant_pub_crate`. The owning `clippy::cargo` and `clippy::nursery` groups are enforced from the Justfile for the same reason.
- `#[allow(...)]` and `#[expect(...)]` are not permitted in authored code. Split or redesign the code instead.
- If custom cfgs are introduced, register them with `cargo::rustc-check-cfg` in `build.rs` or the manifest lint configuration. Do not silence `unexpected_cfgs`.
- Prefer `#[must_use]` for important return values and `pub(crate)` for internal APIs.
- FFI crates may omit a crate-wide `forbid(unsafe_code)` if necessary, but unsafe code must stay isolated to the documented boundary modules and shims. Do not use lint suppressions to permit unsafe.
- `just test`, `just test-features-min`, `just test-native`, `just db-migrate`, `just cov`, and `just validate` now default `REVAER_TEST_DATABASE_URL` to the Postgres maintenance database at `postgres://revaer:revaer@localhost:5432/postgres`. `just db-start` also retries transient Postgres recovery/startup/not-yet-accepting-connection errors around `sqlx migrate run` and `sqlx database reset` before treating the database as mismatched. Keep recipes and docs aligned with that admin-connection workflow when test database bootstrapping changes.
- `just cov` records coverage once with `cargo llvm-cov --workspace --all-features --no-report`, then enforces the 90% per-package line threshold with `cargo llvm-cov report --package ...` against that shared workspace dataset. Keep the coverage gate workspace-sourced so library crates receive credit for lines exercised by downstream crates and integration tests.

# Documentation

- Public crates need crate-level rustdoc that explains purpose, invariants, and a realistic usage example.
- Externally consumed public items should document:
  - behavior
  - invariants and assumptions
  - error cases
  - panic behavior, if any exists
  - copy-pasteable examples when the item is meant to be used directly
- Prefer examples that use `?` and explicit error handling over `unwrap()`/`expect()`.

# Maintainability And Layout

- Keep files single-purpose and cohesive.
- Target roughly 300-400 non-test LOC per production file. Split large files instead of silencing `too_many_lines`.
- `lib.rs` should stay limited to crate docs, module declarations, light re-exports, and tiny crate-boundary glue.
- `main.rs` must remain a thin bootstrap entry point.
- Name modules for what they own. No grab-bag files mixing domain logic, transport DTOs, adapters, and orchestration.

# Performance

- Measure before optimizing. Do not land “performance” refactors based on taste or folklore.
- For performance-driven changes, record the command, benchmark, trace, or timing report that justified the change in the task record or ADR.
- Prefer simpler, more explicit code unless measurement shows a real hotspot.
