# Nexus + DaisyUI Dashboard UI Checklist

## 1) UX constraints and source-of-truth rules
- [ ] Treat `ui_vendor/nexus-html@3.1.0/src` as markup reference only; do not run any Node tooling.
- [ ] Treat `static/nexus` as the runtime asset kit (compiled CSS, images, client-side JS).
- [ ] Use DaisyUI Blueprint MCP as the canonical reference for DaisyUI v5 component patterns, variants, accessibility, and form layout; if Nexus markup differs, Nexus wins for visual parity unless intentionally standardizing.

## 2) Theme + palette (from the logo)
- [ ] Establish a Revaer theme that leans into dark navy/purple with magenta-purple accent.
- [ ] Use token guidance from the logo: background ~#000030 to #000040; accent ~#6A0071; primary ~#901CBB; highlight ~#C42AC3.
- [ ] Apply tokens via DaisyUI theme variables or theme selection plus minimal overrides; keep contrast readable for table/list UIs.

## 3) Shell and routing (atomic composition)
- [ ] Implement AppShell template: sidebar + topbar + main outlet; preserve Nexus layout structure.
- [ ] Routes: Torrents (default), Categories, Tags, Settings, Health.
- [ ] Torrents route supports deep linking to selected torrent (open details drawer) via URL state.

## 4) Authentication and first-run flow
- [ ] On startup, determine configured vs not configured.
- [ ] If not configured: force Setup flow (blocking screen).
- [ ] If configured and auth missing: show auth prompt (blocking modal/screen).
- [ ] Setup flow: call `admin/setup/start`; handle "already configured" by switching to configured state.
- [ ] Setup flow: collect setup token and complete via `admin/setup/complete`.
- [ ] After completion, route to auth prompt.
- [ ] Auth prompt (configured state): provide two tabs: API key, or Local auth (username/password).
- [ ] Store auth choice in local storage and attach to all API requests.
- [ ] API key maps to header `x-revaer-api-key` (per OpenAPI).
- [ ] Local auth sends `Authorization: Basic ...` (server may ignore).
- [ ] Settings allow "bypass local" toggle if present; when enabled default auth prompt to API key.

## 5) Rust UI component library (atomic + props discipline)
- [ ] Build primitives: Buttons (variants, sizes, loading, icon slots), IconButton.
- [ ] Build primitives: Inputs (text, password, number), SearchInput with debounce.
- [ ] Build primitives: Select/MultiSelect, Checkbox/Toggle.
- [ ] Build primitives: Badge, Progress, Tooltip, Skeleton, EmptyState.
- [ ] Build primitives: Dropdown menu, Tabs.
- [ ] Build primitives: Modal, Drawer (details panel), Toast/Alert.
- [ ] Build primitives: Table/List row components and a sticky bulk action bar.
- [ ] Every component exposes all configurables as props: labels, counts, state, href, ids, optional sections, variants, and an extra class hook.

## 6) API client layer (typed, centralized errors)
- [ ] Build a typed client module for OpenAPI-backed endpoints.
- [ ] Endpoints: `GET /health`, `GET /health/full`.
- [ ] Endpoints: `GET /metrics` (optional viewer).
- [ ] Endpoints: `GET /v1/torrents`, `POST /v1/torrents`.
- [ ] Endpoints: `GET /v1/torrents/{id}`.
- [ ] Endpoints: `POST /v1/torrents/{id}/action`.
- [ ] Endpoints: `PATCH /v1/torrents/{id}/options`.
- [ ] Endpoints: `POST /v1/torrents/{id}/select`.
- [ ] Endpoints: `GET /v1/torrents/categories`; `PUT /v1/torrents/categories/{name}`.
- [ ] Endpoints: `GET /v1/torrents/tags`; `PUT /v1/torrents/tags/{name}`.
- [ ] Endpoints: `POST /v1/torrents/create`.
- [ ] Endpoints: `GET /v1/torrents/events` (SSE).
- [ ] Centralize error parsing with ProblemDetails; display status/title/detail consistently.
- [ ] Implement rate limit handling (429) with user-visible backoff messaging.

## 7) Torrents list page (main screen)
- [ ] Layout: Nexus dashboard styling, list-based view, with filter bar and FAB.
- [ ] Filters in URL query: query text (name), state, tags, tracker, extension.
- [ ] Pagination: limit and cursor; provide Load more using next cursor.
- [ ] Columns (TorrentSummary): name, state, progress, down/up rate, ratio, tags, trackers, updated timestamp.
- [ ] Row click opens details drawer and updates route (deep link).
- [ ] Row menu actions: pause, resume, reannounce, recheck, sequential toggle.
- [ ] Row menu actions: set rate (download/upload bps).
- [ ] Row menu actions: remove (confirm + delete_data toggle).
- [ ] Bulk operations: multi-select checkboxes and bulk action bar.
- [ ] Bulk ops: issue /{id}/action per selected torrent in parallel with concurrency cap.
- [ ] Bulk ops: collect per-item failures, show summary toast, keep drawer closed unless single selection remains.
- [ ] Bulk actions: pause, resume, recheck, reannounce.
- [ ] Bulk actions: sequential on/off, rate set, remove (confirm + optional delete_data).

## 8) Torrent details drawer (tabs: Overview, Files, Options)
- [ ] Overview: summary fields + same actions; show last error if present.
- [ ] Files tab: render TorrentFile list; include/exclude and priority edits.
- [ ] Files tab: updates via `POST /v1/torrents/{id}/select` (support skip_fluff toggle if desired).
- [ ] Options tab: render TorrentSettingsView; only editable fields that map to `PATCH /v1/torrents/{id}/options`.
- [ ] Options tab: read-only settings shown as static rows (no fake toggles).

## 9) FAB actions (Torrents screen)
- [ ] Add torrent modal supports magnet or metainfo_b64.
- [ ] Add torrent modal always generates id client-side (UUID v4).
- [ ] Add torrent modal allows initial tags/category and initial rate limits if supported.
- [ ] Create torrent modal wired to `POST /v1/torrents/create`.
- [ ] Create torrent modal provides copy buttons for magnet/metainfo.
- [ ] Provide shortcuts to manage categories and tags.

## 10) Categories and Tags pages (policy management)
- [ ] Categories list and editor: list existing, create/update via PUT with TorrentLabelPolicy fields.
- [ ] Categories editor: structured form with Advanced section for rarely used policy fields.
- [ ] Tags list and editor: same pattern as categories.

## 11) Health page (operator-facing)
- [ ] Show `/health` basic status.
- [ ] Show `/health/full`: degraded components, version/build info, and any torrent snapshot fields.
- [ ] Optional: `/metrics` viewer with copy button.

## 12) Live updates over SSE (robust, header-capable)
- [ ] Implement SSE using fetch streaming and manual SSE parsing (EventSource cannot set headers).
- [ ] Attach `x-revaer-api-key` to SSE requests.
- [ ] Endpoint discovery: primary `/v1/torrents/events`.
- [ ] Fallback: attempt `/v1/events/stream` if primary 404s.
- [ ] Parse SSE fields: event, id, retry, data.
- [ ] Attempt JSON parse of data; if fail, treat as plain text.
- [ ] If JSON includes torrent id, update row; otherwise trigger throttled list refresh (<= 1-2s).
- [ ] UI: show "Live" indicator when connected and subtle warning when reconnecting/backing off.

## 13) Not in scope guardrails
- [ ] Do not implement qBittorrent compatibility endpoints.
- [ ] Do not add node/npm/vite/tailwind build steps.
- [ ] Do not add fake UI controls for settings that cannot be persisted via existing endpoints.
