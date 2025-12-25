# UI torrent FAB + create modals

- Status: Accepted
- Date: 2025-12-24
- Context:
  - The torrent UX checklist requires FAB-driven add/create modals and initial rate limits.
  - API calls must stay in the app layer with shared DTOs, and UI state lives in yewdux.
- Decision:
  - Implement a floating action button that opens Add and Create torrent modals.
  - Wire POST `/v1/torrents/create` through the ApiClient and surface results + copy actions.
  - Move UI preferences (mode/density/locale) into the shared store for consistent access.
  - Alternatives considered:
    - Keep the add panel inline in the list view (rejected; no FAB flow).
    - Let modal components call the API directly (rejected; breaks layering rules).
- Consequences:
  - Adds modal UX for torrent add/authoring and a FAB entry point.
  - Introduces minimal new store state for create results/errors and busy flags.
  - Additional translations and CSS required for modal + FAB presentation.
- Follow-up:
  - Validate Add/Create modals visually against Nexus styling.
  - Run full `just ci` and confirm zero warnings.

## Motivation
- Finish the remaining torrent UX checklist items for FAB actions and authoring flows.
- Keep state management consistent with the yewdux store rule.

## Design notes
- Modal components remain pure UI: they emit typed requests and copy intents via callbacks.
- Create results are stored in the torrents slice to avoid cross-component ad hoc state.

## Test coverage summary
- Unit tests updated for add payload validation (rate parsing).
- No new integration tests for UI-only changes.

## Observability updates
- None (UI-only change).

## Risk & rollback plan
- Risk: modal flows may need styling adjustments across breakpoints.
- Rollback: revert UI modal/FAB changes and the create endpoint wiring.

## Dependency rationale
- No new dependencies introduced; reused existing shared DTOs and UI helpers.
