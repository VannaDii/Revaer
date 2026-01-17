# 086 - Default Local Auth Bypass (Task Record)

- Status: Accepted
- Date: 2026-01-17

## Motivation
- Ensure factory reset remains available when configuration data is broken.
- Default new installs to a recoverable auth state without implicit API key setup.

## Design Notes
- Switch `AppAuthMode` default to `none` and align setup completion fallback.
- Change the `app_profile.auth_mode` database default to `none` via migration.
- Make setup helpers send explicit `auth_mode` values for both auth paths.
- Update reference configuration documentation to match the new default.

## Decision
- Default auth mode to no-auth in code and migrations, while leaving explicit API key setups unchanged.

## Consequences
- New databases start with no-auth access until setup selects API key mode.
- Existing databases retain their configured auth mode unless reset.

## Test Coverage Summary
- Existing API/E2E flows cover both auth modes; setup helper now sets auth mode explicitly.

## Observability Updates
- None.

## Risk & Rollback
- Risk: integrations relying on implicit API key setup must now send `auth_mode` explicitly.
- Rollback: revert the auth mode defaults and migration; restore previous setup fallback.

## Dependency Rationale
- No new dependencies introduced.
