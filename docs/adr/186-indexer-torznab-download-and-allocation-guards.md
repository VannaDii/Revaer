# Indexer Torznab download and allocation guards

- Status: Accepted
- Date: 2026-02-04
- Context:
  - We need Torznab download redirects to complete core ERD coverage and satisfy PR review feedback.
  - Allocation safety must rely on live memory information and apply to request-driven allocations.
  - Review feedback also called for clearer validation structure in bootstrap secrets.
- Decision:
  - Add a stored-procedure-backed Torznab download prepare path that validates instance/profile/tag access and records acquisition attempts.
  - Extend allocation guards to all request-dependent allocations, including Torznab XML escaping, and clamp vector capacities to bounded limits.
  - Refactor secret env validation to a shared helper for consistency.
- Consequences:
  - Positive outcomes:
    - Torznab clients can request download redirects with audited acquisition attempts.
    - Allocation safety applies uniformly and relies on live memory data.
    - Validation logic is more maintainable and easier to test.
  - Risks or trade-offs:
    - Allocation checks can reject requests when memory telemetry is unavailable or too low.
- Follow-up:
  - Continue Torznab search response coverage and add richer download telemetry once search is implemented.

## Task record

- Motivation: close Torznab download gap, address allocation safety/GHAS feedback, and tighten secret validation.
- Design notes:
  - Download path uses a stored procedure to enforce profile/tag rules and populate acquisition_attempt.
  - Allocation checks use live system memory and guard XML escaping plus request-sized collections.
  - Secret env validation is centralized to avoid duplication and preserve constant error messages.
- Test coverage summary: `just ci` (fmt/lint/udeps/audit/deny/test/cov/build-release) and `just ui-e2e`.
- Observability updates: none; existing spans and error context fields remain the primary signals.
- Risk & rollback plan: revert migration 0078 and API handlers, then reset DB migrations; no data migrations beyond new procs.
- Dependency rationale: no new dependencies; `bytes` updated to 1.11.1 to address RustSec advisory.
