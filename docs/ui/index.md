# Web UI - Phase 1

Rust/Yew UI for the Phase 1 torrent workflow. The goal is a responsive, touch-friendly surface that stays usable on 360px phones through 4K desktops while handling large torrent libraries.

- **Pages**: Dashboard, Torrents (list + detail), Logs, Health, Settings.
- **Modes**: Simple (trimmed controls) and Advanced (full controls). Stored in local storage.
- **Transport**: REST for initial payloads; fetch-based SSE for live updates and logs (header-auth supported, EventSource not used).

## Layout and breakpoints

| Name | Width | Default behaviors |
| --- | --- | --- |
| xs | 0-479px | Card view for torrents, drawer navigation, stacked dashboard cards |
| sm | 480-767px | Card view, two-column stats grid inside cards |
| md | 768-1023px | Compact table, tabbed detail view |
| lg | 1024-1439px | Full table, fixed sidebar |
| xl | 1440-1919px | Split panes and wider tables |
| 2xl | 1920px+ | Ultra-wide tables with capped text widths |

Table responsiveness: required columns (Name, Status, Progress, Down, Up) stay pinned; ETA, Ratio, Size, Tags, Path, Updated collapse into overflow or the detail drawer when space is constrained.

Detail view: mobile renders tabs (Overview, Files, Options); desktop promotes a split layout that keeps overview and options visible together at lg+.

Virtualization: the torrent list uses a windowed renderer to keep large libraries responsive; selection stays highlighted for keyboard actions.

## Auth and setup

- API key auth is default. The UI prompts for `key_id:secret` and stores it in local storage with expiry metadata.
- If `app_profile.auth_mode` is `none` and the request originates from a local network, the UI can enter anonymous mode.
- Setup mode guides the operator through the setup token flow and stores the generated API key after completion.

## Transport and SSE

- Primary SSE: `/v1/torrents/events` with filters for torrent id, event kind, and state.
- Fallback SSE: `/v1/events/stream` if the primary endpoint is unavailable.
- Logs stream: `/v1/logs/stream`.
- SSE requests attach `x-revaer-api-key` and `Last-Event-ID` headers.

## Settings coverage

Settings tabs are grouped into: Downloads, Seeding, Network, Storage, Labels, and System. Each tab reflects the corresponding config section and validation errors from `ProblemDetails` responses.

## Theming and localization

- Theme tokens and layout variables live in `static/style.css`.
- Theme selection follows OS preference on first load and persists to local storage.
- Locale selector uses JSON bundles in `i18n/` with English fallback and RTL hinting.

## Running the UI

- Crate: `crates/revaer-ui` (Yew + wasm).
- Commands: `just ui-serve` to preview, `just ui-build` for release builds.
- Assets: `static/style.css` holds palette/breakpoints; `index.html` + `Trunk.toml` bootstrap trunk.
