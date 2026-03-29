# 244. Indexer health event drill-down

- Status: accepted
- Date: 2026-03-17

## Motivation

- `ERD_INDEXERS_CHECKLIST.md` still leaves the health and notifications parity slice unchecked.
- Operators already have connectivity rollups and reputation summaries, but they still lack a direct read path for raw `indexer_health_event` rows defined by `ERD_INDEXERS.md`.
- The next efficient step is to expose recent health events end-to-end so the existing `/indexers` console can show failure detail and conflict timing without introducing a larger notification system yet.

## Design notes

- Add stored procedures `indexer_health_event_list_v1(...)` and stable wrapper `indexer_health_event_list(...)` to read recent events for one indexer instance with actor validation and bounded limits.
- Extend the data, app, and API layers with typed health-event list reads and a new `GET /v1/indexers/instances/{indexer_instance_public_id}/health-events` route.
- Extend the indexer admin UI with a health-event limit field, fetch action, and rendered drill-down cards under the connectivity section.
- Keep notification delivery out of scope for this slice; the checklist item remains open until delivery hooks exist.

## Test coverage summary

- Added stored-procedure tests for recent-row ordering and missing-instance failure mapping.
- Added API handler tests for successful health-event reads and conflict mapping.
- Extended API and UI Playwright smoke coverage for the new health-event surface.

## Observability updates

- No new emitters were added; this slice reads the existing `indexer_health_event` diagnostic stream already populated by backend workflows.
- The new API route reuses existing request tracing and metrics middleware.

## Risk & rollback plan

- Risk is limited to a new read-only proc and route plus UI rendering.
- Rollback is straightforward: revert the migration, API handler/route, and UI panel if operator output regresses.

## Dependency rationale

- No new dependencies were added.
