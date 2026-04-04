# 225: Indexer unit test domain coverage

- Status: Accepted
- Date: 2026-03-01
- Context:
  - ERD checklist requires explicit unit-test coverage across canonicalization, policy evaluation, category mapping, and search validation domains.
  - Coverage existed across multiple modules but the checklist item remained open.
- Decision:
  - Confirm and document active coverage for these four domains in `revaer-data` indexer tests.
  - Mark the checklist item complete based on verified test coverage.
- Consequences:
  - Positive outcomes:
    - Checklist state now matches implemented and exercised tests.
    - Coverage expectation for core indexer rule domains is explicitly recorded.
  - Risks or trade-offs:
    - This records current scope; future rule additions still require new tests.
- Follow-up:
  - Keep extending domain tests as ERD behavior expands.

## Motivation
- Keep ERD checklist status aligned with actual test enforcement in CI.

## Design notes
- Canonicalization coverage includes fallback identity, rollup median behavior, append-only paging, and conflict logging.
- Policy evaluation coverage includes request policy drops and policy match logic.
- Category mapping coverage validates upsert/delete and invalid mapping paths.
- Search validation coverage exercises identifier mismatch, season/episode rules, and category filter validation.

## Test coverage summary
- Verified coverage in `crates/revaer-data/src/indexers/canonical.rs`, `crates/revaer-data/src/indexers/search_results.rs`, `crates/revaer-data/src/indexers/policy_match.rs`, `crates/revaer-data/src/indexers/category_mapping.rs`, and `crates/revaer-data/src/indexers/search_requests.rs`.
- Will run `just ci`, `just build-release`, and `just ui-e2e` before hand-off.

## Observability updates
- No telemetry changes.

## Risk & rollback plan
- If coverage assertions become inaccurate, reopen checklist and add missing tests.
- Roll back by reverting checklist/ADR updates if the requirement is re-scoped.

## Dependency rationale
- No new dependencies added.
