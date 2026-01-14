# 078 - Local Auth Bypass Guardrails (Task Record)

- Status: In Progress
- Date: 2026-01-11

## Motivation
- Stop offering anonymous access when the backend does not allow no-auth mode.
- Ensure disabling local auth bypass requires credentials so operators cannot lock themselves out.

## Design Notes
- Track backend auth_mode from /.well-known and config snapshot updates; allow anonymous only when auth_mode is none and the UI host is local.
- When no-auth is enabled and no credentials exist, set Anonymous auth state to connect immediately.
- When no-auth is disabled while anonymous, clear anonymous state and re-open the auth prompt.
- Guard settings changes that switch auth_mode to api_key unless API key or local auth credentials are saved.

## Decision
- Gate anonymous UI behavior on backend auth_mode + local host detection.
- Block config saves that disable bypass without saved credentials.

## Consequences
- Anonymous access is only offered when the backend explicitly allows it on a local host.
- Operators must save credentials before switching to auth-required mode.

## Test Coverage Summary
- Added unit tests for AuthState credential validation.

## Observability Updates
- None (UI-only change).

## Risk & Rollback
- Risk: remote UI access to no-auth servers now requires credentials despite server allowing none.
- Rollback: revert auth_mode gating in app shell and the settings guard.

## Dependency Rationale
- No new dependencies introduced.
