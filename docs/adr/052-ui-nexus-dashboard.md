# UI dashboard migration to Nexus vendor layout

- Status: Accepted
- Date: 2025-12-25
- Context:
  - Align the dashboard and shell UI with the vendored Nexus HTML to remove drift.
  - Remove the blocking SSE overlay and replace it with a non-blocking connectivity surface.
  - Preserve routing and layout classes so Nexus CSS can remain authoritative.
- Decision:
  - Replace the old dashboard and shell markup with Nexus vendor partials and dashboard structure.
  - Introduce SSE connectivity state in the store with a drawer-footer indicator and modal.
  - Remove legacy dashboard CSS overrides and ensure vendor app.css is the primary styling source.
- Consequences:
  - Positive: Nexus parity, simpler shell structure, non-blocking connectivity UX.
  - Risks: UI copy/labels diverge from vendor defaults; mode toggle now relies on existing stored preference.
- Follow-up:
  - Verify visual parity against Nexus dashboard sections.
  - Monitor SSE reconnection details surfaced in the modal.

## Motivation
- Ensure the UI matches the vendored Nexus dashboard and shell while eliminating legacy layout glue.
- Replace blocking SSE overlays with a navigation-safe connectivity indicator.

## Design notes
- App shell and dashboard markup map directly to `ui_vendor/nexus-html@3.1.0` partials and the ecommerce dashboard page.
- Dashboard sections are split into Nexus-faithful organisms while preserving class names and nesting.
- SSE status is stored in `system.sse_status`; indicator consumes a summary slice, modal consumes full details.

## Test coverage summary
- `just ci` (fmt, lint, udeps, audit, deny, ui-build, test, cov)

## Observability updates
- None.

## Risk & rollback plan
- If Nexus markup causes regressions, revert to the previous dashboard/shell and reintroduce the prior CSS and route wiring.
- If SSE diagnostics cause UI noise, hide the indicator by feature flag and keep reconnect logic intact.

## Dependency rationale
- Added `web-sys` feature `HtmlDialogElement` to open the Nexus search modal via `show_modal` without new crates.
