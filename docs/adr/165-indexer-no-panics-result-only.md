# Indexer result-only returns and no-panics verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: Phase 6 requires no panics/unwrap/expect in production paths and
    Result-only returns for fallible operations.
  - Constraints: preserve existing interfaces and keep verification scoped to
    indexer runtime modules.
- Decision:
  - Audited indexer-related modules for `panic!`, `unwrap()`, `expect()`,
    `unreachable!()` in non-test code and found none.
  - Verified fallible operations return `Result<T, E>`; `Option<T>` usage is limited
    to non-fallible accessors and optional payloads.
  - Alternatives considered: expanding the audit to the entire workspace; deferred to
    avoid blocking indexer-phase progress.
- Consequences:
  - Positive: checklist item satisfied for indexer runtime paths without code churn.
  - Risks/trade-offs: future modules must keep the same constraints; broader workspace
    audit remains out of scope for this ADR.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` passed (npm audit still reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: no new spans/metrics needed for this verification step.
  - Risk & rollback plan: documentation-only change; revert ADR/checklist updates if
    verification is found incomplete.
  - Dependency rationale: no new dependencies added.
