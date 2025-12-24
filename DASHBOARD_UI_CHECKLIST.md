# Nexus + DaisyUI Dashboard UI Checklist

## 1) UX constraints and source-of-truth rules
- [ ] Treat ui_vendor/nexus-html@3.1.0/src as markup reference only; do not run any Node tooling.
- [ ] Treat static/nexus as the runtime asset kit (compiled CSS, images, client-side JS).
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
- [ ] Setup flow: call admin/setup/start; handle "already configured" by switching to configured state.
- [ ] Setup flow: collect setup token and complete via admin/setup/complete.
- [ ] After completion, route to auth prompt.
- [ ] Auth prompt (configured state): provide two tabs: API key, or Local auth (username/password).
- [ ] Store auth choice in local storage and attach to all API requests.
- [ ] API key maps to header x-revaer-api-key (per OpenAPI).
- [ ] Local auth sends Authorization: Basic ... (server may ignore).
- [ ] Settings allow "bypass local" toggle; when enabled default auth prompt to API key and avoid showing local auth first.

## 5) State management and rendering performance (yewdux)
- [ ] Adopt yewdux for all shared UI/data state; avoid use_reducer + ContextProvider for shared data.
- [x] Define a single normalized AppStore with domain sub-structs:
  - [x] auth (configured, setup status, auth method, key/user presence, last auth error)
  - [x] ui (theme, toasts, modal/drawer state, FAB open, busy flags)
  - [x] torrents (normalized maps, filters, paging, selection, details cache)
  - [x] labels (categories/tags caches)
  - [x] health (basic/full snapshots)
  - [x] system (system rates, SSE connection status)
- [ ] Normalize torrent data:
  - [x] torrents.by_id: HashMap<Uuid, Rc<TorrentRowState>>
  - [x] torrents.visible_ids: Vec<Uuid> (render list by IDs only)
  - [x] torrents.selected: HashSet<Uuid> (bulk)
  - [x] torrents.filters: TorrentsQueryModel (mirrors URL query)
  - [x] torrents.paging: { cursor, next_cursor, limit, is_loading }
  - [x] torrents.details_by_id: HashMap<Uuid, Rc<TorrentDetailState>> (optional cache; keep large vectors here, not in row state)
  - [ ] torrents.fsops_by_id: HashMap<Uuid, Rc<FsopsState>> (separate map; row derives a small badge slice)
- [ ] Implement selectors for row-level subscription:
  - [x] select_visible_ids()
  - [x] select_torrent_row(id) (for drawer)
  - [x] select_torrent_progress_slice(id) (for list rows; minimal fields only)
  - [x] select_is_selected(id) (for bulk checkbox)
  - [x] select_system_rates() and select_sse_status()
- [x] Ensure selector return values are cheap and stable:
  - [x] Use Rc/Arc for row models and replace only the changed row pointer on updates.
  - [x] Derive/implement PartialEq so unchanged slices do not trigger re-render.
  - [x] Keep visible_ids as IDs; do not copy full row structs into list state.
- [x] Row list components must subscribe only to progress/state slices, not full TorrentRowState, unless rendering the drawer.

## 6) API client layer (singleton + shared domain types)
- [x] Implement a single ApiClient instance (created once) and share via a lightweight context ApiCtx.
- [x] Enforce: no component constructs its own API client; all API calls go through the singleton.
- [x] Restrict API calls to page controllers/services (effects/hooks/modules), not atoms/molecules.
- [x] Atoms and molecules must never perform API calls or dispatch side effects.
- [x] ApiClient must use existing domain types from shared backend crates (models, enums, request/response types); do not recreate parallel UI-only types.
- [x] Prefer re-exporting or directly depending on shared crates for:
  - [x] TorrentSummary, TorrentDetail, TorrentSettingsView, TorrentFile, TorrentLabelPolicy
  - [x] EventEnvelope, Event, TorrentState, related enums
- [ ] Build only minimal transport/adaptation glue (headers, auth, pagination, SSE), not duplicate schemas.
- [x] Endpoints: GET /health, GET /health/full.
- [x] Endpoints: GET /metrics (optional viewer).
- [x] Endpoints: GET /v1/torrents, POST /v1/torrents.
- [x] Endpoints: GET /v1/torrents/{id}.
- [x] Endpoints: POST /v1/torrents/{id}/action.
- [ ] Endpoints: PATCH /v1/torrents/{id}/options.
- [ ] Endpoints: POST /v1/torrents/{id}/select.
- [ ] Endpoints: GET /v1/torrents/categories; PUT /v1/torrents/categories/{name}.
- [ ] Endpoints: GET /v1/torrents/tags; PUT /v1/torrents/tags/{name}.
- [ ] Endpoints: POST /v1/torrents/create.
- [x] Endpoints: GET /v1/torrents/events (SSE).
- [x] Centralize error parsing with ProblemDetails; display status/title/detail consistently.
- [x] Implement rate limit handling (429) with user-visible backoff messaging and safe retry.

## 7) SVG and icon system (reuse + consistency)
- [ ] Encapsulate every SVG as a Yew component under atoms/icons/*.
- [ ] Icon component props: size, class, optional title, optional variant (outline/solid) when relevant.
- [ ] Build IconButton and standardize hover/active/focus states with DaisyUI Blueprint patterns.
- [ ] Replace all inline SVG usage in pages/components with the icon components.

## 8) Rust UI component library (atomic + props discipline)
- [ ] Build primitives: Buttons (variants, sizes, loading, icon slots), IconButton.
- [ ] Build primitives: Inputs (text, password, number), SearchInput with debounce.
- [ ] Build primitives: Select/MultiSelect, Checkbox/Toggle.
- [ ] Build primitives: Badge, Progress, Tooltip, Skeleton, EmptyState.
- [ ] Build primitives: Dropdown menu, Tabs.
- [ ] Build primitives: Modal, Drawer (details panel), Toast/Alert.
- [ ] Build primitives: Table/List row components and a sticky bulk action bar.
- [ ] Every component exposes all configurables as props: labels, counts, state, href, ids, optional sections, variants, and an extra class hook.

## 9) Torrents list page (main screen)
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

## 10) Torrent details drawer (tabs: Overview, Files, Options)
- [ ] Overview: summary fields + same actions; show last error if present.
- [ ] Files tab: render TorrentFile list; include/exclude and priority edits.
- [ ] Files tab: updates via POST /v1/torrents/{id}/select (support skip_fluff toggle if desired).
- [ ] Options tab: render TorrentSettingsView; only editable fields that map to PATCH /v1/torrents/{id}/options.
- [ ] Options tab: read-only settings shown as static rows (no fake toggles).
- [ ] Details caching discipline: keep large file vectors/settings off the hot row model; update drawer from details_by_id.

## 11) FAB actions (Torrents screen)
- [ ] Add torrent modal supports magnet or metainfo_b64.
- [ ] Add torrent modal always generates id client-side (UUID v4).
- [ ] Add torrent modal allows initial tags/category and initial rate limits if supported.
- [ ] Create torrent modal wired to POST /v1/torrents/create.
- [ ] Create torrent modal provides copy buttons for magnet/metainfo.
- [ ] Provide shortcuts to manage categories and tags.

## 12) Categories and Tags pages (policy management)
- [ ] Categories list and editor: list existing, create/update via PUT with TorrentLabelPolicy fields.
- [ ] Categories editor: structured form with Advanced section for rarely used policy fields.
- [ ] Tags list and editor: same pattern as categories.

## 13) Health page (operator-facing)
- [ ] Show /health basic status.
- [ ] Show /health/full: degraded components, version/build info, and any torrent snapshot fields.
- [ ] Optional: /metrics viewer with copy button.

## 14) Live updates over SSE (robust, header-capable, envelope-first)
- [x] Implement SSE using fetch streaming and manual SSE parsing (EventSource cannot set headers).
- [x] Attach x-revaer-api-key to SSE requests.
- [x] Endpoint discovery: primary /v1/torrents/events.
- [x] Fallback: attempt /v1/events/stream if primary 404s.
- [x] Parse SSE fields: event, id, retry, data.
- [x] Treat the canonical payload as EventEnvelope { id, timestamp, event } JSON; decode this first.
- [x] Back-compat decode: if envelope parse fails, attempt { kind, data } dummy payload mapping to an internal envelope shape.
- [x] All SSE events must be normalized into a single internal EventEnvelope shape and applied through a single reducer path.
- [x] Persist and send Last-Event-ID header for replay; store last numeric event id after successful envelope decode.
- [x] Build SSE query filters from UI state:
  - [x] torrent = comma-separated visible IDs when count is below a safe cap; otherwise omit.
  - [x] event = kinds relevant to current view (list vs drawer).
  - [x] state only when a state filter is active.
- [x] Implement a progress coalescer:
  - [x] Buffer incoming progress patches per torrent ID in a non-reactive buffer.
  - [x] Flush buffered progress into yewdux store on a fixed cadence (50-100ms).
  - [x] Apply flush by replacing only the affected Rc<TorrentRowState> entries.
  - [x] Apply non-progress events immediately (add/remove/state change/metadata/fsops/files/selection/system/health).
  - [x] If an event cannot be decoded/applied, trigger a throttled targeted refresh (do not refetch everything per message).
- [x] Progress coalescing is mandatory to cap render frequency under high event volume.
- [x] UI: show "Live" indicator when connected and subtle warning when reconnecting/backing off.

## 15) Not in scope guardrails
- [ ] Do not implement qBittorrent compatibility endpoints.
- [ ] Do not add node/npm/vite/tailwind build steps.
- [ ] Do not add fake UI controls for settings that cannot be persisted via existing endpoints.
