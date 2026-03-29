# Indexer PR Feedback Allocation Follow-up

- Status: Accepted
- Date: 2026-02-02
- Context:
  - Review feedback highlighted unbounded allocations in indexer handlers and asked for clearer,
    live-memory guardrails.
  - Allocation safety needed to remain cross-platform and avoid hard-coded assumptions.
  - Test helper naming and error diagnostics in tests required clarification.
- Decision:
  - Add explicit allocation safety checks for request-driven string and vector allocations using
    the shared live-memory guard.
  - Introduce a minimum-available-memory threshold and a cached-system entry point to avoid
    repeated probing where reuse is possible.
  - Rename shared indexer test state helper and tighten ProblemDetails parsing in tests.
- Consequences:
  - Safer handling of request-sized allocations with clearer memory-policy documentation.
  - Improved test helper clarity and more actionable test failures.
  - Slightly more allocation checks per request, offset by the option to reuse a system snapshot.
- Follow-up:
  - Confirm code scanning alerts clear after the next GitHub Advanced Security scan.
  - No migrations required.

## Motivation

Close PR feedback on allocation safety and test clarity while keeping indexer handler behavior
intact and aligned with live-memory guardrails.

## Design notes

- Allocation sizing now checks request-derived bytes against live available memory before
  materializing strings or vectors.
- The allocation guard exposes a cached-system entry point and enforces a minimum available
  memory threshold before allowing allocations.
- Shared indexer test helpers use clearer naming and explicit expectations for response decoding.

## Test coverage summary

- `just ci`
- `just build-release`
- `just ui-e2e`

## Observability updates

- None (guardrails and test refactors only).

## Risk & rollback plan

- Low risk: behavior is additive and defensive. Roll back by reverting this change set if
  allocation checks prove too strict in practice.

## Dependency rationale

- No new dependencies added.
