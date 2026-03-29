# Indexer search request auth E2E coverage

- Status: Accepted
- Date: 2026-02-04
- Context:
  - Enforce the v1 rule that REST search requests and canonical torrent source access require API key auth.
  - Validate behavior in E2E tests without reducing existing indexer functionality.
- Decision:
  - Add E2E coverage for search request create/cancel auth requirements.
  - Add E2E coverage for Torznab download behavior when the apikey is missing.
  - Keep API responses and handler behavior unchanged; tests only validate the existing contract.
- Consequences:
  - Positive outcomes: regression coverage for auth enforcement on search requests and Torznab downloads.
  - Risks or trade-offs: additional E2E runtime and reliance on seeded system actor for search requests.
- Follow-up:
  - Monitor CI E2E stability after adding coverage.

## Motivation

Search request endpoints and Torznab downloads must enforce API key authentication consistently. We need explicit E2E coverage to guard against regressions while retaining current behavior.

## Design notes

- Use existing API fixtures to test authenticated and unauthenticated flows.
- Verify missing apikey returns HTTP 401 for Torznab downloads.
- Keep tests scoped to public endpoints and avoid relying on external indexer data.

## Test coverage summary

- Added E2E tests for search request create/cancel auth behavior.
- Added E2E test for missing apikey on Torznab download.

## Observability updates

- No new telemetry; tests validate existing API responses.

## Risk & rollback plan

- Low risk: test-only changes. If tests are unstable, revert the E2E additions and re-evaluate fixtures or environment setup.

## Dependency rationale

- No new dependencies.
