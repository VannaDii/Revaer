# Indexer tryOp wrappers for external operations

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: Phase 6 requires wrapping external/system calls in tryOp-style helpers
    to normalize error mapping across indexer data access.
  - Constraints: panics are forbidden; do not introduce new dependencies; keep SQL
    interactions confined to stored-procedure calls.
- Decision:
  - Introduce a shared `try_op` helper in the data layer and replace per-file
    `map_query_err` closures across indexer modules.
  - Use `try_op` in all indexer data-layer SQLx interactions (queries, executes, and
    row extraction) to standardize error mapping.
  - Note: panic catching is intentionally not used because `catch_unwind` is banned and
    production code must avoid panics entirely.
  - Alternatives considered: leave per-file closures or introduce a more complex async
    wrapper; rejected in favor of a simple, centralized helper.
- Consequences:
  - Positive: consistent error mapping for indexer data access and fewer duplicate
    helper definitions.
  - Risks/trade-offs: none beyond standard refactor risk; behavior remains equivalent.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` passed (npm audit still reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: no new spans/metrics required for this refactor.
  - Risk & rollback plan: revert the try_op refactor and restore per-module helpers if
    regressions appear.
  - Dependency rationale: no new dependencies added.
