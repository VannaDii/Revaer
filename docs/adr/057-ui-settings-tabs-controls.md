# UI settings tabs and editor controls

- Status: Accepted
- Date: 2025-12-28
- Context:
  - The settings screen exposed raw configuration values without meaningful grouping or editing controls.
  - Torrent operators need quick access to download, seeding, network, and storage controls with clear defaults.
  - Settings patches must flow through the existing API and honor immutable fields.
- Decision:
  - Rebuild the settings UI as tabbed panels aligned with torrent workflows (connection, downloads, seeding, network, storage, system).
  - Drive all editable controls from the config snapshot and submit targeted `/v1/config` changesets per group.
  - Treat immutable fields and effective engine snapshots as read-only with copy-to-clipboard affordances.
- Consequences:
  - Settings are now grouped for faster navigation and support direct edits with consistent controls.
  - The UI performs more client-side validation for numeric and JSON fields before patching.
- Follow-up:
  - Evaluate dedicated server-side directory browsing if operators need richer path discovery.
  - Add localization for settings field labels where needed.

## Motivation
- Make settings usable for torrent operators by grouping them into purpose-built tabs.
- Replace raw config tables with toggles, selects, numeric inputs, and path pickers.
- Ensure read-only values are still accessible via copy actions.

## Design notes
- Draft values are derived from the latest config snapshot and compared to build minimal changesets.
- Immutable keys from `app_profile.immutable_keys` and derived engine fields are rendered read-only.
- Directory selection uses a modal picker with suggested paths from the snapshot.

## Test coverage summary
- `just ci`

## Observability updates
- UI toasts surface config patch failures and copy failures; no new metrics.

## Risk & rollback plan
- Risk: incorrect grouping or input parsing could lead to failed patches.
- Rollback: revert to the previous settings view and re-fetch configuration.

## Dependency rationale
- No new dependencies.
