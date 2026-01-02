# Dashboard UI checklist completion and auth/SSE hardening

- Status: Accepted
- Date: 2026-01-01

## Motivation
- Complete remaining dashboard UI checklist items without adding new dependencies.
- Tighten auth and SSE handling to avoid stale tokens and replay conflicts.

## Context
- UI relies on SSE for live torrent updates and must survive Last-Event-ID conflicts.
- Auth tokens require a 14-day TTL enforced by both server and client.
- UI should allow anonymous mode when server auth_mode is none.

## Decision
- Move torrent sort state into URL-backed filters and apply client-side ordering.
- Reset SSE Last-Event-ID on 409 conflict and reconnect with backoff.
- Refresh API keys on save to capture expiry; invalidate keys on logout via config patch.
- Mirror CORS origin on the API router to cover SSE and REST.

Alternatives considered:
- Add a dedicated logout endpoint: rejected to avoid OpenAPI changes.
- Store API keys without expiry: rejected to enforce 14-day TTL.

## Design Notes
- Sorting is represented as `sort=key:dir` in the query string.
- Metadata updates trigger a targeted list refresh to keep tags/trackers current.
- Anonymous auth is enabled from `.well-known` app_profile when configured.

## Consequences
- Login now performs a refresh call to capture expiry; failures surface as toasts.
- Some SSE metadata events trigger list refreshes, increasing fetch volume.

## Test Coverage Summary
- `DATABASE_URL=postgres://revaer:revaer@172.17.0.1:5432/revaer REVAER_TEST_DATABASE_URL=postgres://revaer:revaer@172.17.0.1:5432/revaer just ci`
  (fmt, lint, udeps, audit, deny, ui-build, test, test-features-min, cov).

## Observability Updates
- No new metrics or tracing changes.

## Risk & Rollback Plan
- Risk: logout fails if config patch is rejected; UI now reports an error toast.
- Rollback: revert UI auth/SSE changes and re-run `just ci`.

## Dependency Rationale
- Updated `sqlx` to 0.9.0-alpha.1 and aligned vendored `hashlink` to hashbrown 0.16 to
  satisfy `clippy::multiple_crate_versions` without introducing git dependencies.

## Follow-up
- Confirm auth refresh behavior against expired keys during QA.
