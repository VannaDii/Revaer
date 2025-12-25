# 044 - UI ApiClient Torrent Options/Selection Endpoints (Task Record)

- Status: In Progress
- Date: 2025-12-24

## Motivation
- Add the remaining torrent options/selection endpoints to the ApiClient.
- Keep transport wiring centralized in the API service layer.

## Design Notes
- Use existing API model types (`TorrentOptionsRequest`, `TorrentSelectionRequest`).
- Keep methods in `services::api::ApiClient` and reuse existing auth/application patterns.

## Decision
- Add ApiClient helpers for options updates and file selection updates.
- Maintain consistent error wrapping and headers via the shared helpers.

## Consequences
- UI features can call these endpoints without duplicating transport logic.
- File selection toggles now persist via the selection endpoint.

## Test Coverage Summary
- API client additions only; no new Rust tests added.

## Observability Updates
- None.

## Risk & Rollback
- Risk: API failures require reloading detail data to reconcile file selection state.
- Rollback: remove the selection update path and ApiClient methods.

## Dependency Rationale
- No new dependencies introduced.
