# 083 - API Preflight Before UI E2E (Task Record)

- Status: Accepted
- Date: 2026-01-14

## Motivation
- Verify API availability before UI E2E runs to reduce false attribution to the UI.

## Design Notes
- Add a dedicated Playwright project that hits public API endpoints.
- Make browser projects depend on the API project to enforce ordering.
- Keep checks read-only and stable: `/health`, `/metrics`, `/docs/openapi.json`.

## Decision
- Add an API preflight spec and wire it as a dependency for UI projects.
- Add `E2E_API_BASE_URL` to the test configuration docs.

## Consequences
- UI tests do not run if API preflight fails.
- E2E setup now needs the API base URL to be accurate.

## Test Coverage Summary
- `just ui-e2e`

## Observability Updates
- None.

## Risk & Rollback
- Risk: API endpoint changes will require updates to the preflight checks.
- Rollback: remove the API project dependency and preflight spec.

## Dependency Rationale
- No new dependencies; reuse Playwright.
