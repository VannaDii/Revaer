# 043 - UI Settings Bypass Local Auth Toggle (Task Record)

- Status: In Progress
- Date: 2025-12-24

## Motivation
- Provide a settings control for preferring API keys in the auth prompt.
- Close the remaining setup/auth flow requirement in the dashboard checklist.

## Design Notes
- Store the bypass toggle in LocalStorage but read/write it only from `app`.
- Keep the settings view stateless and driven by AppStore props.
- Use the toggle to influence the default auth prompt tab without forcing logout.

## Decision
- Add a settings feature view for the bypass local toggle.
- Persist the toggle separately from the last-used auth mode.

## Consequences
- Auth prompt defaults to API key when bypass is enabled.
- Existing auth state remains unchanged unless the user re-authenticates.

## Test Coverage Summary
- UI-only change; no new Rust tests added.

## Observability Updates
- None.

## Risk & Rollback
- Risk: users may still remain logged in with local auth while bypass is enabled.
- Rollback: remove the settings view and toggle wiring.

## Dependency Rationale
- No new dependencies introduced.
