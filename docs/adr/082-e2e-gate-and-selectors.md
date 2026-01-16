# 082 - E2E Gate and Selector Stability (Task Record)

- Status: Accepted
- Date: 2026-01-14

## Motivation
- Stabilize Playwright selectors against shared nav labels and auth overlays.
- Make UI E2E runs a required quality gate for local changes.
- Document how to run the E2E suite.

## Design Notes
- Scope selectors to the layout content area or sidebar to avoid strict-mode collisions.
- Use the auth overlay's dismiss icon button when present; fall back to the text button.
- Document the `just ui-e2e` requirement in README and AGENT.

## Decision
- Update Playwright page objects to scope selectors and handle the auth overlay deterministically.
- Add UI E2E requirements to `README.md` and `AGENT.md`.

## Consequences
- Navigation and logs checks avoid ambiguous label matches.
- E2E tests are enforced as a local quality gate.

## Test Coverage Summary
- `just ui-e2e`

## Observability Updates
- None.

## Risk & Rollback
- Risk: UI label changes may still require selector updates.
- Rollback: revert the selector scoping and gate requirements.

## Dependency Rationale
- No new dependencies; reuse Playwright and dotenv.
