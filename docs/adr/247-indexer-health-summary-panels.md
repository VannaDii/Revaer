# 246. Indexer health summary panels

- Status: accepted
- Date: 2026-03-16

## Motivation

- The indexer admin console could fetch connectivity profiles and source-reputation rows, but operators only saw those responses in the generic activity log.
- `ERD_INDEXERS_CHECKLIST.md` still leaves the health dashboard slice open because the UI was missing the visible status badges and summary panels described by `ERD_INDEXERS.md`.
- The next efficient step is to render those existing API reads directly in `/indexers` so operators can review health state without leaving the page or parsing raw JSON logs.

## Design notes

- Add local UI state for the latest connectivity profile and fetched reputation rows alongside the existing health-event state.
- Render a connectivity summary card with a status badge, dominant error, latency bands, and recent success-rate snapshots.
- Render source-reputation cards for the selected window and keep health-event drill-down unchanged.
- Leave notification delivery out of scope for this slice; the health checklist item remains open until email/webhook hooks exist.

## Test coverage summary

- Added unit coverage for connectivity badge-class mapping and percent formatting helpers in the indexer UI logic module.
- Extended the `/indexers` route smoke test to assert the new health summary headings render.
- Full `just ci` and `just ui-e2e` remain the end-to-end verification gates.

## Observability updates

- No new emitters were added.
- This slice improves operator visibility by presenting already-collected connectivity and reputation telemetry directly in the admin console.

## Risk & rollback plan

- Risk is limited to UI state/rendering changes over existing API calls.
- Rollback is a straightforward revert of the new state/rendering helpers and task-record updates if the console regresses.

## Dependency rationale

- No new dependencies were added.
