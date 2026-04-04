# Indexer CF state E2E coverage

- Status: Accepted
- Date: 2026-01-30
- Context:
  - Motivation: satisfy UI E2E API coverage gate for newly added CF state endpoints.
  - Constraints: no new dependencies; reuse existing E2E API fixtures and coverage hooks.
- Decision:
  - Extend indexer instance E2E API coverage to hit CF state GET and reset endpoints using a missing-instance 404 path.
  - Alternatives considered: add a dedicated fixture to create a real instance (rejected for higher setup cost in current E2E suite).
- Consequences:
  - Positive: coverage gate includes CF state endpoints and remains green.
  - Trade-offs: responses are 404-only in this test until instance creation is wired into E2E fixtures.
- Follow-up:
  - Expand E2E to exercise CF state success paths once instance creation fixtures are available.

## Test Coverage

- `just ci`
- `just ui-e2e`

## Observability

- No changes.

## Risk and Rollback

- Risk: minimal; only exercises API endpoints in E2E.
- Rollback: revert `tests/specs/api/indexers-instances.spec.ts` additions.

## Dependency Rationale

- No new dependencies added.
