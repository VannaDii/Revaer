# UI: Hardline Nexus Dashboard Rebuild and Settings Wiring

- Status: Accepted
- Date: 2025-12-26
- Context:
  - The Home dashboard must match the vendored Nexus HTML structure and DaisyUI component patterns.
  - Navigation and shell need to be simplified to Home/Torrents/Settings with a non-blocking SSE indicator.
  - Settings must remain reachable even when auth is missing and show a config snapshot.
- Decision:
  - Rebuild dashboard sections to mirror Nexus markup (stats cards, storage status, recent events, tracker health, queue summary).
  - Align AppShell sidebar/topbar with Nexus partial structure and move the SSE indicator to the sidebar footer.
  - Wire Settings to fetch `/v1/config` and provide test-connection actions while keeping auth overlays off the Settings route.
  - Disable wasm-opt in the Trunk pipeline (`data-wasm-opt="0"`) to avoid build failures on missing staged wasm outputs.
  - Use relative static asset paths for Nexus CSS and dashboard image URLs to keep styles/images loading when served from non-root paths.
  - Alternatives considered: importing `revaer_config::ConfigSnapshot` into the UI; rejected to avoid new cross-crate dependencies in wasm.
- Consequences:
  - Positive: consistent Nexus/DaisyUI layout, simplified nav, and settings access even during auth errors.
  - Trade-offs: UI-only fetches rely on runtime connectivity; config display is untyped JSON in the UI; wasm bundles are no longer optimized by wasm-opt.
- Follow-up:
  - Verify visual parity in the browser and keep the Nexus HTML deltas minimal.
  - Add typed config rendering if a UI-safe shared type becomes available.

## Task Record
- Motivation: enforce Nexus + DaisyUI parity for the dashboard while keeping Settings reachable and diagnostics visible.
- Design notes: mapped each dashboard section to specific Nexus blocks; SSE indicator uses sidebar footer with a non-blocking dialog; config snapshot parsed as `serde_json::Value` to avoid new dependencies; disabled wasm-opt in `crates/revaer-ui/index.html` to keep `trunk build --release` reliable on this environment until tooling changes; adjusted Nexus asset URLs to relative paths for more reliable static hosting.
- Test coverage summary: UI changes rely on existing CI gates; no new unit tests added.
- Observability updates: none (UI-only changes).
- Risk & rollback plan: revert `crates/revaer-ui` dashboard/shell/settings edits and `static/style.css` if UI regressions appear.
- Dependency rationale: no new dependencies added; reused existing `serde_json`.
