# Indexer routing policy service and endpoints

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: expose routing policy create/param/secret operations through the indexer
    application facade and HTTP surface per ERD Phase 6/8 requirements.
  - Constraints: stored-procedure-only DB access, constant error messages, DI-only
    wiring in bootstrap, no new dependencies.
- Decision:
  - Extend the indexer facade with routing policy operations and implement them in
    `revaer-app` using existing stored-procedure wrappers.
  - Add routing policy request/response DTOs plus HTTP handlers and routes for
    create, parameter set, and secret binding.
  - Update the OpenAPI document to describe the new endpoints and schemas.
- Consequences:
  - Positive: routing policy operations are now available to API callers with
    consistent error mapping and tracing spans.
  - Risks/trade-offs: additional endpoints increase API surface and will require
    follow-on list/update/delete support to be feature complete.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` (npm audit reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: added spans for routing policy operations; no new metrics yet.
  - Risk & rollback plan: revert the routing policy service/API changes if
    regressions appear; stored procedures remain unchanged.
  - Dependency rationale: no new dependencies added (used existing crates only).
