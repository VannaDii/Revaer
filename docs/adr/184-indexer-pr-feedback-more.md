# Indexer PR Feedback Follow-up (Allocation Caps)

- Status: Accepted
- Date: 2026-02-02
- Context:
  - Additional PR feedback requested explicit caps for request-driven allocations and safer test
    body parsing.
  - Allocation guards must use live memory data while still providing deterministic upper bounds.
- Decision:
  - Add explicit maximum sizes for search profile domain/tag keys and policy rule text inputs.
  - Limit test response body reads to a fixed upper bound.
  - Document the secret key ID max-length source for maintainability.
- Consequences:
  - Reduced risk of unbounded allocations from large inputs.
  - Clearer operational limits with minimal user-facing constraints.
  - Test helpers avoid excessive memory use on malformed responses.
- Follow-up:
  - Confirm GHAS/code scanning alerts clear after the next scan.
  - No migrations required.

## Motivation

Ensure indexer handlers enforce conservative, explicit input caps alongside live-memory guards and
improve test safety for large responses.

## Design notes

- Search profile domain keys and tag keys now have maximum counts and per-key byte limits.
- Policy rule text inputs (including value set items) enforce per-field byte limits.
- Test helper response parsing reads at most 1 MiB.

## Test coverage summary

- `just ci`
- `just ui-e2e`

## Observability updates

- None.

## Risk & rollback plan

- Low risk: validation rejects overlarge inputs up front. Roll back by reverting this change set
  if limits are too strict.

## Dependency rationale

- No new dependencies added.
