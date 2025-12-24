# UI SSE normalization, progress coalescing, and ApiClient singleton

- Status: Accepted
- Date: 2025-12-24
- Context:
  - The UI SSE pipeline needed legacy payload normalization, replay support, and render-friendly progress handling.
  - App state was still split across `use_state`, and API clients were being constructed per call.
  - The dashboard checklist requires a single SSE reducer path and a singleton ApiClient via context.
- Decision:
  - Normalize SSE payloads into `UiEventEnvelope` and route all updates through one reducer path in the app shell.
  - Persist and replay `Last-Event-ID`, add SSE query filters derived from store state, and coalesce progress updates on a fixed cadence.
  - Introduce an `ApiCtx` context that owns a single `ApiClient` instance with mutable auth state.
  - Move auth/torrents/system SSE state into the yewdux `AppStore` and update reducers accordingly.
  - Store bulk-selection state in `AppStore` via a shared `SelectionSet` to keep bulk actions consistent across views.
  - Patch the `anymap` dependency used by `yewdux` to avoid Rust 1.91 auto-trait pointer cast errors.
- Consequences:
  - SSE progress events are buffered and flushed together, reducing render churn during bursts.
  - API calls now share a single client instance, simplifying auth updates and call sites.
  - Bulk selections now persist in store state, avoiding local-only checkbox state drift.
  - Additional store slices (UI/labels/health) remain future work; some UI state still uses local hooks.
- Follow-up:
  - Expand `AppStore` to include UI/toast/health/labels slices and row-level selectors.
  - Add coverage for SSE filtering and progress coalescer cadence.

## Motivation

- Align the UI with the SSE checklist requirements and remove per-call ApiClient construction.

## Design notes

- SSE decoding emits `UiEventEnvelope` instances; `handle_sse_envelope` is the only reducer entry.
- Progress patches are stored in a non-reactive `HashMap` and flushed every 80ms into `AppStore` via `apply_progress_patch`.
- `ApiCtx` holds a single `ApiClient`; auth changes update the shared `RefCell` state.
- Bulk selection updates are routed through `SelectionSet` so store mutations remain deterministic.

## Test coverage summary

- `just ci` (fmt, lint, check-assets, udeps, audit, deny, ui-build, test, test-features-min, cov).

## Observability updates

- No new telemetry; SSE connection state continues to drive the existing UI overlay.

## Risk & rollback plan

- Risk: SSE filter mismatches could drop events; fallback is the throttled refresh path.
- Rollback: revert to the previous SSE handler and local state wiring, removing the coalescer and ApiCtx usage.

## Dependency rationale

- Added a workspace dependency on `revaer-events` for shared SSE types; patched `anymap` locally for Rust 1.91 compatibility.
- Alternatives considered: keep UI-local event types or upgrade `yewdux` (requires Yew 0.21+); rejected to avoid duplicating schemas or triggering a larger UI upgrade.
