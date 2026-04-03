# AGENT.MD — Codex Operating Instructions (Revaer, Rust 2024)

> **Prime Directives**
>
> 1. **Rust 2024 only**. Never lower the edition.
> 2. **No dead code**. No unused items, no future stubs, no parking-lot code.
> 3. **Minimal dependencies**. Prefer `std`; every new dependency needs written rationale.
> 4. **The Justfile is law**. Local and CI build/test/lint/release gates run through [`justfile`](/Users/vanna/Source/revaer/justfile).
> 5. **Stored procedures or bust**. Runtime database access goes through stored procedures; raw SQL belongs only in migrations and tightly scoped operational bootstrap scripts.
> 6. **Deterministic, panic-free production code**. No `panic!`, `unwrap()`, `expect()`, `unreachable!()`, or silent error suppression in authored production or bootstrap code.
> 7. **No source-level lint suppressions**. `#[allow(...)]` and `#[expect(...)]` are not permitted in authored code.
> 8. **`just ci` and `just ui-e2e` before every hand-off**. A task is not complete until both pass cleanly.
> 9. **Dependencies are injected**. Runtime logic receives collaborators from callers; only bootstrap/wiring code constructs concrete implementations or reads the environment.
>
> **Completion Rule:** Because Codex runs locally, a task is complete only when all requirements in this file and the scoped instruction files are satisfied, `just ci` passes without warnings or errors, and `just ui-e2e` passes.

---

## 0) Policy Precedence And Source Of Truth

- [`AGENTS.md`](/Users/vanna/Source/revaer/AGENTS.md) is the non-negotiable root contract.
- Scoped instruction files under [`.github/instructions/`](/Users/vanna/Source/revaer/.github/instructions) may only tighten or specialize the root contract for their matching paths. They may not relax root policy.
- If two instruction files appear to conflict, precedence is:
  1. [`AGENTS.md`](/Users/vanna/Source/revaer/AGENTS.md)
  2. the most specific scoped instruction file
  3. supporting docs and ADRs
- Operational source-of-truth files are:
  - [`justfile`](/Users/vanna/Source/revaer/justfile)
  - [`.github/workflows/ci.yml`](/Users/vanna/Source/revaer/.github/workflows/ci.yml)
  - [`.github/workflows/pr.yml`](/Users/vanna/Source/revaer/.github/workflows/pr.yml)
  - [`.github/workflows/sonar.yml`](/Users/vanna/Source/revaer/.github/workflows/sonar.yml)
  - [`sonar-project.properties`](/Users/vanna/Source/revaer/sonar-project.properties)
- This file must reference those operational files instead of copying large command bodies or stale workflow inventories.
- Current scoped instruction files:
  - [`.github/instructions/rust.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/rust.instructions.md)
  - [`.github/instructions/revaer-data.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/revaer-data.instructions.md)
  - [`.github/instructions/revaer-ui.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/revaer-ui.instructions.md)
  - [`.github/instructions/ffi.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/ffi.instructions.md)
  - [`.github/instructions/devops.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/devops.instructions.md)
  - [`.github/instructions/sonarqube_mcp.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/sonarqube_mcp.instructions.md)

---

## 1) Repository Invariants

- Keep the repo library-first. Binaries are thin bootstrap/wiring layers; reusable logic lives in library crates.
- Keep the public API small. Use `pub(crate)` by default and expose cross-crate items intentionally.
- Runtime database access uses stored procedures only. No runtime inline SQL outside the migration/operational exceptions called out in scoped instructions.
- `JSONB` and other conglomerate persistence formats are banned for application state. Persist normalized data.
- Runtime collaborators are injected. Do not read environment variables or construct concrete infra implementations inside domain logic.
- Zero dead code is mandatory. If code ships, it is exercised in production, tests, or an explicitly exercised feature configuration.
- Temporary operational exceptions, such as duplicate-crate tolerances in [`deny.toml`](/Users/vanna/Source/revaer/deny.toml) or advisory ignores in [`.secignore`](/Users/vanna/Source/revaer/.secignore), must be explicit, ADR-backed, time-bounded, and kept outside authored source code.

---

## 2) Authored Code Quality Posture

- Edition and MSRV are pinned by [`Cargo.toml`](/Users/vanna/Source/revaer/Cargo.toml) and [`rust-toolchain.toml`](/Users/vanna/Source/revaer/rust-toolchain.toml). Keep them aligned.
- Treat warnings as errors across the workspace. Do not weaken lint posture in source or ad hoc commands.
- Authored production and bootstrap code must not panic. Return errors explicitly and terminate cleanly at the top-level boundary.
- Silent error suppression is forbidden. Handle, translate, or propagate every fallible operation.
- Log errors at their origin point once. Do not re-log the same error as it travels up the call chain.
- `Option<T>` is allowed only for legitimate absence semantics or partial-function domains where `None` is the complete, expected result.
- `Result<T, E>` is required for recoverable failure. Do not hide failure in `Option`, booleans, sentinel values, or logs.
- `std::panic::catch_unwind` is forbidden everywhere except documented FFI boundary shims covered by [`.github/instructions/ffi.instructions.md`](/Users/vanna/Source/revaer/.github/instructions/ffi.instructions.md).
- If a rule cannot be satisfied cleanly, redesign, split, delete, or isolate the code behind the documented FFI boundary. Do not silence the rule.

---

## 3) Quality Gates

- All local and CI operations run through `just` recipes. Workflows may install tools or stage artifacts, but build/test/lint/release gates must call `just`.
- Handoff requires:
  - `just ci`
  - `just ui-e2e`
- [`justfile`](/Users/vanna/Source/revaer/justfile) is the canonical command surface for build, lint, test, coverage, release, docs, and local dev loops.
- [`ci.yml`](/Users/vanna/Source/revaer/.github/workflows/ci.yml), [`pr.yml`](/Users/vanna/Source/revaer/.github/workflows/pr.yml), and [`sonar.yml`](/Users/vanna/Source/revaer/.github/workflows/sonar.yml) must stay aligned with the Justfile and this policy.
- Sonar analysis scope and first-party signal shaping are versioned in [`sonar-project.properties`](/Users/vanna/Source/revaer/sonar-project.properties). Keep PR quality-gate behavior and new-code policy aligned with that file.

---

## 4) Maintainability Guardrails

- Keep one canonical statement of each global rule. Root policy belongs here; scoped files should reference it and add path-specific details instead of repeating or rewording it inconsistently.
- Any change to [`justfile`](/Users/vanna/Source/revaer/justfile), workflow files, release scripts, lint posture, or [`sonar-project.properties`](/Users/vanna/Source/revaer/sonar-project.properties) must update the relevant instruction file in the same change.
- Review the instruction set whenever crate layout, workflow layout, release flow, or quality gates change materially.
- Keep user-facing docs, examples, and generated API/reference artifacts in sync when exposed surfaces change.

---

## 5) Task Record And ADR Rules

- Every task persists a task record alongside the change as an ADR under [`docs/adr/`](/Users/vanna/Source/revaer/docs/adr).
- Start from [`docs/adr/template.md`](/Users/vanna/Source/revaer/docs/adr/template.md), number sequentially, and keep the file name concise and searchable.
- Every task record must include:
  - Motivation
  - Design notes
  - Test coverage summary
  - Observability updates
  - Risk and rollback plan
  - Dependency rationale
  - Stale-policy check
- The stale-policy check must record:
  - which instruction files were reviewed
  - whether drift was found
  - which contradictions or stale references were removed
- Update [`docs/adr/index.md`](/Users/vanna/Source/revaer/docs/adr/index.md) and [`docs/SUMMARY.md`](/Users/vanna/Source/revaer/docs/SUMMARY.md) in the same change that adds the ADR.
