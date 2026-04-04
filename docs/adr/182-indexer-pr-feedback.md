# Indexer PR Feedback Follow-through

- Status: Accepted
- Date: 2026-02-01
- Context:
  - Addressed open PR feedback on indexer handlers, allocation safety, and API request shape.
  - Needed clearer documentation for session encryption env vars and allocation limits.
  - Reduced duplicated test scaffolding while preserving testability and coverage.
- Decision:
  - Centralize allocation safety in a helper, apply it to request-driven allocations, and document the 80% safety limit.
  - Consolidate indexer handler test scaffolding into a shared test helper module.
  - Move string normalization helpers into a shared indexer module.
  - Remove redundant indexer instance public ID from the update request body.
- Consequences:
  - Clearer memory allocation policy and safer handling of unbounded inputs.
  - Leaner test modules with shared helpers and fewer duplicated imports.
  - API request shape aligns with path-based identifiers, reducing ambiguity.
- Follow-up:
  - Monitor code scanning to confirm allocation alerts clear after rescans.
  - No additional migrations required.

## Motivation

Align indexer handler code with review feedback, improve allocation safety for user-driven inputs,
reduce test duplication, and clarify API request semantics.

## Design notes

- Allocation helpers now gate request-sized buffers using live memory data and a documented 80%
  cap to preserve headroom.
- A test support module centralizes stub config and response parsing helpers for indexer handler
  tests without exposing them outside the indexers module.
- String normalization helpers are shared across indexer handlers to avoid duplication.
- `IndexerInstanceUpdateRequest` now relies solely on path identifiers.

## Test coverage summary

- `just ci`
- `just build-release`
- `just ui-e2e`

## Observability updates

- None (documentation-only changes and refactors).

## Risk & rollback plan

- Low risk: changes are additive or refactor-only. Roll back by reverting the individual commits
  if any regression is observed.

## Dependency rationale

- No new dependencies added in this change set; see ADR 180 for the live-memory probe rationale.
