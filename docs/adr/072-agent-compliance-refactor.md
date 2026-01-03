# 072: Agent Compliance Refactor (UI + HTTP + Config Layout)

- Status: Accepted
- Date: 2026-01-03
- Context:
  - Motivation: bring the repository into closer alignment with AGENT layout and tooling rules after drift in UI routing, HTTP module layout, and config structure.
  - Constraints: preserve existing APIs/behavior while relocating modules; avoid new dependencies and keep stored-procedure-only database access intact.
- Decision:
  - Design notes: move torrent UI views into the feature module, scope window/router usage to the app layer, and reorganize API HTTP handlers/DTOs into `handlers/` and `dto/` while re-exporting to preserve public paths.
  - Alternatives considered: leave modules in place and document exceptions (rejected to keep the structure enforceable); introduce a large-scale API surface rename (rejected to avoid breaking changes).
- Consequences:
  - Positive outcomes: clearer module boundaries, AGENT-compliant Justfile/CI flow, and reduced cross-layer coupling in the UI.
  - Risks or trade-offs: short-term churn from file moves and import updates; slight increase in module indirection via re-exports.
- Follow-up:
  - Test coverage summary: `just ci` (fmt, lint, udeps, audit, deny, ui-build, test, test-features-min, cov, build-release) passed with the ≥80% line coverage gate satisfied.
  - Observability updates: no new spans or metrics added for this refactor.
  - Risk & rollback plan: revert the module move commits and restore prior paths if regressions appear; no data migrations were introduced.
  - Dependency rationale: no new dependencies added; alternatives were to add helper crates for routing/structure, which were rejected to keep the footprint minimal.
