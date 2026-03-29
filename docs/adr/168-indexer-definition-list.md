# Indexer definition list endpoint

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: expose the indexer definition catalog via the API so UI/CLI flows can
    enumerate definitions without leaking internal IDs.
  - Constraints: stored-procedure-only DB access, constant error messages, injected
    dependencies, and no new dependencies.
- Decision:
  - Add a stored procedure to list indexer definitions with actor validation.
  - Wire a data-layer wrapper, application facade method, and HTTP handler to return
    definition summaries.
  - Document the new endpoint and DTOs in OpenAPI and add API coverage.
- Consequences:
  - Positive: indexer definitions can be listed through a stable API surface.
  - Risks/trade-offs: only summary data is exposed; follow-on endpoints are still
    needed for field metadata and instance creation flows.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` (npm audit reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: added tracing span for definition listing; no new metrics yet.
  - Risk & rollback plan: revert the definition list service/API changes if
    regressions appear; stored procedures remain additive.
  - Dependency rationale: no new dependencies added (used existing crates only).
