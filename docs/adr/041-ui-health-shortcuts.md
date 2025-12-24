# 041 – UI Health View + Label Shortcuts (Task Record)

- Status: In Progress
- Date: 2025-10-24

## Motivation
- Replace the Health route placeholder with an operator-facing status view built from cached snapshots.
- Provide quick navigation from torrent add flow to label policy management.

## Design Notes
- Implemented a dedicated health feature view that reads from `AppStore` and renders basic/full snapshots plus the raw metrics text.
- Added label shortcuts in the add-torrent panel using router links to avoid side effects in components.

## Decision
- Keep health rendering in a feature view module with no API calls; data remains sourced from app-level effects.
- Use existing chip/button styling patterns for navigation shortcuts.

## Consequences
- Operators can inspect health status without leaving the UI.
- Add-torrent flow now exposes direct navigation to categories and tags.

## Test Coverage Summary
- UI-only additions (no new Rust tests added).

## Observability Updates
- None (UI-only changes, no new telemetry).

## Risk & Rollback
- Risk: health fields may appear empty when snapshots are unavailable; view handles None gracefully.
- Rollback: revert the health feature module and restore the placeholder route.

## Dependency Rationale
- No new dependencies introduced.

## Follow-up
- Add metrics copy controls and align health styling with Nexus patterns.
