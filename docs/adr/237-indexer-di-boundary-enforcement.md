# Indexer DI Boundary Enforcement

- Status: Accepted
- Date: 2026-03-08
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had the dependency-injection boundary item open.
  - The indexer runtime path in `revaer-app` is meant to operate on injected collaborators only, while bootstrap remains the only place allowed to read environment variables and construct concrete infrastructure.
  - This was already mostly true in code, but it was not enforced by tests, so regressions would be easy to introduce.
- Decision:
  - Add architecture tests in `crates/revaer-app/src/bootstrap.rs` that pin the DI boundary for indexer runtime wiring.
  - Assert that `crates/revaer-app/src/indexers.rs` does not read environment variables or construct core infrastructure directly.
  - Assert that `crates/revaer-app/src/bootstrap.rs` remains the place that reads env vars and wires concrete metrics, event bus, runtime state, and `IndexerService`.
- Consequences:
  - The indexer runtime module now has an explicit regression test for the DI rule from `AGENTS.md`.
  - Bootstrap stays the wiring boundary, and service code remains easier to test because collaborators are passed in.
  - The enforcement is intentionally narrow and source-based, so future refactors must keep these invariants visible or update the test with an equivalent wiring design.
- Follow-up:
  - Extend the same pattern to other runtime subsystems if more non-bootstrap wiring starts to accumulate.
  - Keep new indexer-domain services on injected constructors instead of hidden singleton/env access.
