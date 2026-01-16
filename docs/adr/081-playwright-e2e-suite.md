# 081 - Playwright E2E Test Suite (Task Record)

- Status: Accepted
- Date: 2026-01-14

## Motivation
- Add automated UI coverage for core routes and modal flows.
- Centralize E2E configuration in a committed `tests/.env` file.

## Design Notes
- Playwright config reads `tests/.env` for base URL, browser selection, timeouts, and artifacts.
- Tests are grouped by page with page objects and a shared app fixture.
- Assertions focus on stable labels and layout anchors to avoid data coupling.

## Decision
- Add a Playwright test harness under `/tests` with config, fixtures, and page objects.
- Add a `just ui-e2e` recipe to run the suite via the standard workflow.
- Ignore Playwright output directories in `.gitignore`.

## Consequences
- UI smoke checks can be run locally and wired into CI when ready.
- Running the suite requires Node tooling and Playwright browser installs.

## Test Coverage Summary
- Added specs for dashboard, torrents, settings, logs, health, and navigation smoke.

## Observability Updates
- None.

## Risk & Rollback
- Risk: label changes or auth/setup overlays can break selectors.
- Rollback: remove the `/tests` Playwright suite and `ui-e2e` recipe.

## Dependency Rationale
- `@playwright/test`: browser automation and test runner.
- `dotenv`: load environment configuration from `tests/.env`.
