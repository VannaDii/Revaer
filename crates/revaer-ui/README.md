# Revaer Web UI (Phase 1)

Rust + Yew implementation of the Phase 1 Web UI. The crate builds to WebAssembly via `trunk` and ships with the palette, breakpoints, localization bundles, and responsive layouts described in `docs/ui/`.

## Running locally

Use the workspace `just` recipes (preferred):

```bash
just ui-serve
```

This installs the wasm target and trunk (if needed), syncs assets, and serves the UI from `crates/revaer-ui/dist-serve`.

## Frontend asset and component policy

- Client-side JS files are allowed at runtime, but no npm/node/yarn/pnpm/vite/tailwind CLI is permitted in dev, CI, or release pipelines.
- Nexus is treated as a vendored compiled asset kit; consume `app.css` and vendor JS as static files.
- Component structure follows the `components/` + `features/` split; atoms live in `components/atoms` and page-level views live under `features/*/view.rs`.
- JS-dependent Nexus behaviors must be gated behind an explicit prop (for example, `enable_js`) and documented; prefer small, deterministic Yew interactions when possible.

## Layout highlights

- Breakpoints: xs (0-479), sm (480-767), md (768-1023), lg (1024-1439), xl (1440-1919), 2xl (1920+).
- Sidebar collapses to a drawer on mobile and stays fixed on laptop+.
- Torrents render as cards on xs/sm and a sortable table on md+; density toggle adjusts row spacing (compact/normal/comfy).
- Dashboard grid auto-fills from 1-4 columns based on viewport.
- Theme and locale selections persist to local storage.

## Files to know

- `src/app/` - router wiring, auth setup, SSE orchestration.
- `src/core/` - layout logic, breakpoints, UI primitives.
- `src/features/` - per-route state and views (dashboard, torrents, logs, health, settings).
- `src/services/` - REST + SSE clients (no Yew/web-sys code).
- `src/components/` - shared UI atoms and composites.
- `static/style.css` - CSS variables for palette and layout rules.
- `index.html` + `Trunk.toml` - trunk entrypoint and bundler config.

The UI connects to the API via `services/api.rs` and the SSE fetch stream in `app/sse.rs`. Use `docs/ui/` for UX and flow references.
