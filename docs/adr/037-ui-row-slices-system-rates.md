# UI row slices and system-rate store wiring

- Status: Accepted
- Date: 2025-12-24
- Context:
  - The checklist requires row-level selectors and ID-based list rendering to avoid full-row re-renders.
  - System rates must live in the AppStore alongside SSE connection state.
  - UI components should remain free of API side effects while still subscribing to yewdux slices.
- Decision:
  - Add `TorrentRowBase` and `TorrentProgressSlice` selectors and render list rows via ID-based components that subscribe only to slices.
  - Keep bulk selection state in `AppStore` and expose selectors for selection and system rates.
  - Store `SystemRates` in `SystemState` and update it from both dashboard fetches and SSE system-rate events.
- Consequences:
  - List rows re-render only when their slice changes, reducing churn under frequent progress updates.
  - Dashboard throughput metrics now follow store-backed system rates rather than local state copies.
  - Additional store slices (filters, paging, fsops) still need to be implemented.
- Follow-up:
  - Finish remaining torrent state normalization (filters, paging, fsops badges).
  - Add selectors for drawer detail slices and wire remaining list filtering/paging flows.

## Motivation

- Align list rendering with checklist performance constraints and centralize system-rate state in the store.

## Design notes

- `TorrentRowItem` uses `use_selector` to read base/progress slices and selection state per row ID.
- SSE `SystemRates` updates now mutate `AppStore.system.rates` instead of local dashboard state.
- Dashboard panels receive `SystemRates` via props to keep UI components data-driven.

## Test coverage summary

- `just ci` (fmt, lint, check-assets, udeps, audit, deny, ui-build, test, test-features-min, cov).

## Observability updates

- No changes.

## Risk & rollback plan

- Risk: list rows could render blank if selector data goes missing; fallback is the existing refresh flow.
- Rollback: revert to list rendering with full rows and remove the per-row selectors.

## Dependency rationale

- No new dependencies.
