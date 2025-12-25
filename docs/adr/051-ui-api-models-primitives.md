# UI shared API models and UX primitives

- Status: Accepted
- Date: 2025-12-25
- Context:
  - The UI and CLI duplicated health/setup/dashboard DTOs, increasing drift risk against the API.
  - The torrent toolbar and labels views lacked debounced search, multi-select, and reusable empty/bulk primitives.
  - The UI checklist requires shared API models and a component primitive set with prop-driven configuration.
- Decision:
  - Move health, setup-start, and dashboard DTOs into `revaer-api-models` and consume them from the API, UI, and CLI.
  - Add shared UI primitives (SearchInput with debounce, MultiSelect, EmptyState, BulkActionBar) and extend existing inputs/buttons for prop coverage.
  - Refactor torrent filters and label empty state to use the new primitives while retaining text-input fallback for tags when options are unavailable.
- Consequences:
  - Reduces schema drift and keeps response shapes centralized in one crate.
  - Adds new UI primitives that standardize filter toolbars and empty states.
  - The setup-start endpoint now serializes expiration as RFC3339 strings to match shared DTOs.
- Follow-up:
  - Audit remaining UI components for prop completeness and update the checklist item when finished.
  - Re-run the full `just ci` pipeline before final handoff.

## Task record
- Motivation: Eliminate duplicate API DTOs and complete missing UI primitives required by the Torrent UX checklist.
- Design notes: Shared DTOs live in `revaer-api-models`; new primitives live under `components` and are consumed by torrents/labels views to avoid dead code.
- Test coverage summary: Not run in this update (follow-up required per AGENT.md).
- Observability updates: None.
- Risk & rollback plan: Revert to previous DTO structs in API/CLI/UI and restore raw input elements if regressions surface.
- Dependency rationale: No new dependencies added.
