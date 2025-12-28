# Revaer Web UI (Phase 1)

Rust + Yew implementation of the Phase 1 Web UI. The crate builds to WebAssembly via `trunk` and ships with the brand palette, breakpoints, localization bundles, and responsive layouts described in `docs/ui/`.

## Running Locally

1. Install the wasm target and trunk:
   ```bash
   rustup target add wasm32-unknown-unknown
   cargo install trunk
   ```
2. Serve the UI:
   ```bash
   trunk serve --open
   ```
   This reads `Trunk.toml`, builds `revaer-ui` for `wasm32-unknown-unknown`, and watches for changes.

## Frontend Asset & Component Policy

- Client-side JS files are allowed at runtime, but no npm/node/yarn/pnpm/vite/tailwind CLI is permitted in dev, CI, or release pipelines.
- Nexus is treated as a vendored compiled asset kit; consume `app.css` and vendor JS as static files.
- Component structure follows atomic design in `src/components/daisy/{atoms,molecules,organisms,templates}`; page-level views live under `src/features/*/view.rs`.
- For new UI components, consult DaisyUI Blueprint and the Nexus HTML templates; if they differ, Nexus markup wins for visual parity.
- JS-dependent Nexus behaviors must be gated behind an explicit prop (for example, `enable_js`) and documented; prefer small, deterministic Yew interactions when possible.

## Layout Highlights

- Breakpoints: xs (0–479), sm (480–767), md (768–1023), lg (1024–1439), xl (1440–1919), 2xl (1920+).
- Sidebar collapses to a hamburger drawer on mobile; fixed on laptop+.
- Torrents render as a sortable table across breakpoints; density toggle adjusts row spacing (compact/normal/comfy).
- Dashboard grid auto-fills from 1–4 columns based on viewport.
- Theme toggle persists to local storage; defaults to dark via `revaer-dark`/`revaer-light` data-theme.
- Locale selector uses JSON bundles in `i18n/` with English fallback and RTL hinting.

## Files to Know

- `src/lib.rs` - shared tokens, modes, and non-wasm pieces.
- `src/app/mod.rs` - Yew router, mode/theme/locale state, and view wiring.
- `src/components/` - dashboard, shell, and torrent list components.
- `static/style.css` - CSS variables for palette and responsive layout rules.
- `index.html` + `Trunk.toml` - trunk entrypoint and bundler config.

The implementation is intentionally data-light and demo-driven to keep the wasm payload small while the backend contracts evolve. Wire real API + SSE endpoints by swapping the demo data in `dashboard.rs` and `torrents.rs` with REST/SSE adapters.
