# UI detail drawer overview/files/options

- Status: Accepted
- Date: 2025-12-24

## Context
- The torrent detail drawer still exposed legacy peers/trackers/log panes instead of the required overview/files/options layout.
- The UI was maintaining a custom DetailData conversion layer instead of using shared API models.
- The checklist requires edits only for fields supported by PATCH /v1/torrents/{id}/options and real file selection updates.

## Decision
- Render the detail drawer with Overview, Files, and Options tabs and include the same action set as the list rows.
- Store TorrentDetail directly in the detail cache to avoid duplicate UI-only models and conversions.
- Apply file selection changes via /select (include/exclude/priority/skip_fluff) and options changes via /options with optimistic updates.
- Keep non-editable settings read-only to avoid fake controls.

## Consequences
- Removes duplicated detail mapping logic and keeps UI aligned with shared models.
- Detail UI now depends on settings payloads for options and skip-fluff rendering.
- Failed updates require a refresh to reconcile optimistic state.

## Motivation
- Align the UI with the Torrent UX checklist while preserving the thin-client model.

## Design notes
- Detail cache remains in yewdux details_by_id; list rows stay lightweight.
- Components emit callbacks only; API calls remain in app-level handlers.

## Test coverage summary
- Added unit tests for detail selection, priority, skip-fluff, and options updates in torrents state.
- Added a format_bytes unit test for the new size formatter.

## Observability updates
- None (UI-only changes).

## Risk & rollback plan
- Risk: optimistic updates may temporarily show stale settings if the API rejects changes.
- Mitigation: refresh detail on failure.
- Rollback: restore the previous detail component and DetailData mapping.

## Dependency rationale
- Added workspace chrono to revaer-ui runtime deps to build demo detail timestamps.
