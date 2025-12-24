# 040 – UI Label Policies (Task Record)

- Status: In Progress
- Date: 2025-10-24

## Motivation
- Provide first-class category/tag policy management in the UI so operators can apply TorrentLabelPolicy defaults without CLI/API-only workflows.
- Maintain AppStore as the source of truth while avoiding API calls in atoms/molecules.

## Design Notes
- Implemented a dedicated `features/labels` slice with form state that round-trips through `TorrentLabelPolicy`.
- Added a single list + editor page that renders per-kind (categories or tags) with an Advanced section for rarely used fields.
- API upserts are routed through the shared `ApiClient` and update the AppStore label caches on success.

## Decision
- Use `LabelFormState` as the sole UI editing model and convert to `TorrentLabelPolicy` only on save.
- Re-export label policy support types from `revaer-api-models` to keep UI aligned with shared domain types.

## Consequences
- Labels are now editable without leaving the UI; any validation errors are surfaced before calling the API.
- The UI must keep label cache entries updated to prevent stale list rendering.

## Test Coverage Summary
- Added unit tests for label form parsing, cleanup validation, and policy mapping.

## Observability Updates
- None (UI-only changes, no new telemetry).

## Risk & Rollback
- Risk: malformed inputs can still hit the API if not caught locally; server-side validation remains authoritative.
- Rollback: revert the labels feature wiring in `app/mod.rs` and the new feature module.

## Dependency Rationale
- No new dependencies introduced; re-exported existing domain types for UI usage.

## Follow-up
- Expand label editor UX (search/filter, bulk actions) and align styling with Nexus components.
