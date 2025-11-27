# Web UI - Phase 1

Rust/Yew implementation plan for the Phase 1 torrent-only UI described in `REVAER_PHASE1_WEBUX_SPEC.md`. The goal is a responsive, touch-friendly surface that stays usable on 360px phones through 4K desktops while handling 50k+ torrents.

- **Pages:** Dashboard, Torrents (list + detail), Search (shared list), Jobs/Post-processing, Settings, Logs. Indexers/Library stay hidden until Phase 2.
- **Modes:** Simple (default, trimmed controls) and Advanced (all filters, columns, bulk actions). Persisted in local storage without reload.
- **Transport:** REST for initial payloads; SSE for torrents, transfers, queue, jobs, and VPN state with backoff and jitter handling.

## Layout & Breakpoints

| Name | Width | Default behaviors |
| --- | --- | --- |
| xs | 0–479px | Card view for torrents, bottom sheet actions, stacked dashboard cards, hamburger navigation |
| sm | 480–767px | Card view, two-column stats grid inside cards, slide-out navigation |
| md | 768–1023px | Compact table (2–4 columns), tabbed detail view, 2-column dashboard grid |
| lg | 1024–1439px | Full table, fixed sidebar with labels, 3-column dashboard grid |
| xl | 1440–1919px | Adaptive column expansion, split-pane detail, 4-column dashboard grid |
| 2xl | 1920px+ | Ultrawide-friendly table and split panes with max readable line lengths |

Table responsiveness: required columns (Name, Status, Progress, Speed up/down) stay pinned; ETA, Ratio, Size, Tags, Tracker, Path collapse into overflow or the detail drawer when space is constrained. Horizontal scroll must preserve keyboard navigation and roving tabindex.

Detail view: mobile renders tabs (`Files`, `Peers`, `Trackers`, `Log`, `Info`) with accordion file tree; desktop/laptop promotes a split grid showing file tree + metadata alongside peers/trackers/log simultaneously.

Virtualization: torrent list uses a windowed renderer (row-height aware with overscan) to keep 50k+ rows responsive; horizontal scroll remains keyboard-safe and selection stays highlighted for shortcut actions.

Auth: remote mode always requires an API key; prompt stores key in local storage, LAN anonymous mode is allowed only if backend advertises `allow_anonymous`. SSE currently appends the key via querystring (EventSource lacks header support); use TLS and avoid logging URLs in deployment. Live updates: SSE feeds torrent progress/rates plus dashboard rates/queue/VPN status; reconnect badge/overlay surfaces failures and retries.

Add flow: drop `.torrent` or paste magnet/URL with inline validation and error copy; invalid file types are rejected. Submissions post to `/v1/torrents`, surface toast feedback, and refresh the list.
Live sync: SSE streams torrent progress/rates/state plus added/removed events to keep the list aligned without manual refresh.

## Theming & Tokens

- Palette: Primary (`#265D81` base), Secondary (`#775A96`), Accent (`#258BD3`), Neutral 50–900, Success/Warning/Error scales; dark mode uses `background-dark #121417`, `surface-dark #1A1C20`, and text tokens.
- Scale: Spacing 4/8/12/16/24/32; radius 4/8/12; elevation tiers flat/raised/floating; type scale xs–2xl.
- States: focus ring 2px accent-500, hover/pressed darken by one tone; inputs/tables use border tokens.
- Theme selection follows OS preference on first load and persists to local storage; user toggle is always available.

## Localization

- Languages: ar, de, es, hi, it, jv, mr, pt, ta, tr, bn, en, fr, id, ja, ko, pa, ru, te, zh.
- Bundles: JSON files at `crates/revaer-ui/i18n/*.json` with English fallback; dotted keys supported in `TranslationBundle`.
- RTL: bidi layout, mirrored progress bars, reversed file tree, and RTL-aware table alignment. `meta.rtl` in bundles hints at direction and is applied on load.
- Numbers/dates: browser locale by default with user override; binary units for rates/sizes.

## Accessibility & Interaction

- WCAG 2.1 AA: semantic markup, focus-visible rings, high-contrast dark mode, 40px touch targets on mobile, focus traps for drawers/modals.
- Keyboard shortcuts (wired in demo UI): `/` search focus, `j/k` row move, `space` pause/resume, `delete` delete, `shift+delete` delete+data, `p` recheck. Selected row highlights; actions surface in a status banner.
- Screen-reader flow follows DOM order (mobile collapse must not break navigation).
- Confirmations: delete (“Remove torrent ‘<name>’? Files remain on disk”), delete+data (“Remove torrent and delete data? This cannot be undone.”), recheck prompt.
- Mobile action bar: sticky bottom row for Pause/Resume/Delete/More on xs/sm; desktop retains row actions in-table.
- SSE downtime overlay: shows last event timestamp, retry countdown (1s→30s with jitter), reason, and retry button; badge reflects reconnect state in top bar.

## Performance & Resilience

- Target: <300ms initial UI load on modern mobile (cached assets), virtualization for all tables beyond 500 rows, main-thread budget aligned with 50k torrents.
- SSE: event batching and reconnect backoff (1s→30s with jitter), Last-Event-ID awareness, overlay with retry countdown and last event timestamp on disconnect.
- Offline/remote awareness: API key prompt stored locally; remote mode always enforces key even if backend advertises anonymous LAN support.

## Running the UI

- Crate: `crates/revaer-ui` (Yew + wasm).
- Commands: `rustup target add wasm32-unknown-unknown`, `cargo install trunk`, `trunk serve --open` to preview.
- Assets: `static/style.css` holds palette/breakpoints; `index.html` + `Trunk.toml` bootstrap trunk.
- Demo data lives in `components/dashboard.rs` and `components/torrents.rs`; swap in REST/SSE adapters to connect to the backend once available.
