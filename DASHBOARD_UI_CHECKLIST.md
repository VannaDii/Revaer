# Nexus + DaisyUI Dashboard UI Checklist

## Findings (repo deltas to address, ordered by impact)

1. Setup flow is non-blocking (should block). CSS disables pointer events on the overlay but keeps the shell interactive.
    - Evidence: `crates/revaer-ui/static/style.css` `.setup-overlay { pointer-events: none; }` and `.setup-overlay .card { pointer-events: auto; }`
    - Status: resolved. Setup overlay now captures pointer events while keeping the card interactive.
2. Setup completion does not route to auth prompt; it immediately sets `auth.state` (prompt only renders when `auth_state` is None).
    - Evidence: `crates/revaer-ui/src/app/mod.rs` setup completion sets `store.auth.state = Some(...)`; prompt render gate checks `auth_state_value.is_none()`.
    - Status: resolved. Setup completion now surfaces the auth prompt even when the API key is stored.
3. Health page is not routed and full/metrics data never fetched.
    - Evidence: no `/health` route in `crates/revaer-ui/src/app/routes.rs`; API client only has `/health`; `HealthPage` reads `health.full` and `metrics_text` that are never populated.
    - Status: resolved. `/health` is routed and `/health/full` + `/metrics` are fetched/stored.
4. Vendored yewdux is reintroduced as a temporary exception to keep `yew`/`yew-router` on the latest crates.io releases.
    - Evidence: `Cargo.toml` patches `yewdux` to `vendor/yewdux`; `vendor/yewdux/crates/yewdux/src/anymap.rs` exists.
    - Status: accepted exception. Must be removed as soon as upstream releases a compatible crates.io `yewdux` (tracked in ADR 074).
5. SVG/icon system not implemented as described (no `atoms/icons/*` components, no IconButton usage).
    - Evidence: no icons module; UI uses inline Iconify spans (e.g., `crates/revaer-ui/src/components/shell.rs`).
    - Status: resolved. Added `components/atoms/icons` + `IconButton` and replaced all iconify spans.
6. Dashboard storage status dropdown actions missing (Enhance/Insights/Auto Tag/Delete).
    - Evidence: `crates/revaer-ui/src/features/dashboard/disk_usage.rs` lacks any actions menu.
    - Status: resolved. Storage card now renders the Nexus dropdown actions.
7. SSE indicator label shows "Connected" not "Live".
    - Evidence: `crates/revaer-ui/src/components/connectivity.rs` `status_label` maps Connected -> "Connected".
    - Status: resolved. Connected state now displays “Live”.
8. Add/Create torrent modals lack shortcuts to manage categories/tags.
    - Evidence: `crates/revaer-ui/src/features/torrents/view/modals.rs` only shows text inputs.
    - Status: resolved. Added Manage Categories/Tags buttons wired via `on_manage_labels`.
9. Torrent list SSE updates do not update tags/trackers for list rows.
    - Evidence: `apply_sse_envelope` only updates progress/status/name/download_dir; no tags/trackers update path.
    - Status: resolved. Metadata events trigger targeted detail refresh; `upsert_detail` now updates list-row tags/tracker/category.
10. i18n still falls back to default locale/keys (missing keys not surfaced).

-   Evidence: `crates/revaer-ui/src/i18n/mod.rs` falls back to default locale then raw key.
-   Status: resolved. Missing keys now return `missing:{key}` with no default locale fallback.

11. Coverage gate previously failed; `just cov` now clears the 80% gate.

-   Evidence: `just cov` reports TOTAL line coverage 80.44% (region 76.98%) and passes `--fail-under-lines 80`.
-   Highest line deficits (missed/total, coverage):
    -   `crates/revaer-config/src/loader.rs`: 547/2181 (74.92%)
    -   `crates/revaer-fsops/src/service/mod.rs`: 472/1891 (75.04%)
    -   `crates/revaer-torrent-libt/src/worker.rs`: 393/2493 (84.24%)
    -   `crates/revaer-app/src/orchestrator.rs`: 475/1638 (71.00%)
    -   `crates/revaer-app/src/bootstrap.rs`: 255/353 (27.76%)
    -   `crates/revaer-data/src/config.rs`: 290/990 (70.71%)
    -   `crates/revaer-cli/src/cli.rs`: 87/177 (50.85%)
    -   `crates/revaer-ui/tools/asset_sync/src/lib.rs`: 107/289 (62.98%)
-   Status: resolved. `just cov` passes at 80.44% line coverage.

12. Vendored dependencies remain in the repo (hashlink/sqlx-core), which conflicts with the “no vendoring” rule.

-   Evidence: `Cargo.toml` patched `hashlink` to `vendor/hashlink` and `vendor/sqlx-core` existed in-tree.
-   Status: resolved. Removed the `hashlink` patch and deleted those vendored crates; the only remaining vendored dependency is `yewdux` (exception in ADR 074).

13. Git-sourced crates violate the “no git dependencies” rule.

-   Evidence: `Cargo.toml` patched `yew` from git and `crates/revaer-ui/Cargo.toml` used git `yewdux`.
-   Status: resolved. Both now use crates.io releases; `deny.toml` allow-git list cleared.

14. UI dependency versions must stay on crates.io while avoiding duplicate Yew/Gloo versions.

-   Evidence: crates.io `yewdux` 0.11 depends on `yew` 0.21; `yew-router` 0.19 depends on `yew` 0.22, which creates multiple Yew/Gloo versions.
-   Status: resolved by vendoring yewdux to support `yew` 0.22 and aligning `gloo` to 0.11 + `gloo-net` to 0.5.

15. `just ci` fails at `just lint` + `just deny` due to duplicate `hashbrown`/`foldhash` versions.

-   Evidence: `cargo clippy` emits `multiple versions for dependency 'hashbrown'` and `cargo deny` reports duplicate `hashbrown`/`foldhash` entries (SQLx/hashlink vs Yew/indexmap).
-   Status: resolved via exception. `just lint` allows `clippy::multiple_crate_versions`, and `deny.toml` allows duplicate `hashbrown`/`foldhash` until SQLx adopts `hashlink ^0.11` (ADR 076).

16. Undefined Nexus-style classes are used (panel/eyebrow/muted/pill/stacked/label-\*), so UI relies on classes that are not defined in the Nexus CSS or our custom stylesheet.

-   Evidence: `crates/revaer-ui/src/features/labels/view.rs` uses `panel`, `panel-head`, `panel-subhead`, `eyebrow`, `muted`, `pill`, `label-*`, `stacked`; `crates/revaer-ui/src/features/health/view.rs` uses `panel`, `panel-head`, `panel-subhead`, `pill`, `muted`. No matching selectors exist in `crates/revaer-ui/static/nexus/assets/app.css` or `crates/revaer-ui/static/style.css`.
-   Suggested correction: replace these with DaisyUI component classes (`card`, `card-body`, `badge`, `text-base-content/60`, `form-control`, `label-text`, `stack`/`join`/`grid`) or add a minimal CSS bridge that maps the Nexus-style names to DaisyUI tokens.
-   Actual correction: Replace with DaisyUI classes where available, otherwise remove the classes being used that don't have definitions.
-   Status: resolved. Health + Labels views now use DaisyUI cards/badges/utility classes with no undefined class names.

17. Labels editor uses bare `<input>` elements without DaisyUI `input`/`form-control` classes, leading to inconsistent spacing/typography vs the rest of the UI.

-   Evidence: `crates/revaer-ui/src/features/labels/view.rs` input fields are missing DaisyUI classes and do not use shared input components.
-   Suggested correction: switch to `Input` component or wrap inputs with `form-control` + `input input-bordered` + `label-text` patterns.
-   Actual correction: Use only proper DaisyUI form structures, and classes for all forms.
-   Status: resolved. Labels editor now uses `form-control`, `label-text`, `input/select`, and `toggle` classes.

18. Dashboard disk usage tab controls include a stray `false` class and lack proper tab semantics.

-   Evidence: `crates/revaer-ui/src/features/dashboard/disk_usage.rs` uses `<div class="tab false px-3">` instead of button tabs with `role="tab"`, `tab-active`, and `aria-selected`.
-   Suggested correction: use DaisyUI tabs with `button` + `role="tab"` and `tab-active`, or remove tabs if they are static placeholders.
-   Actual correction: Use proper DaisyUI tabs structures and styles.
-   Status: resolved. Tabs now use `button` with `role="tab"`, `aria-selected`, and DaisyUI `tab` classes.

### Remediation tasks (ordered, with acceptance criteria)

-   [x] Setup overlay is truly blocking.
    -   Acceptance: pointer events are captured by the overlay (background shell cannot be clicked), overlay still focuses the card, and setup screen visually remains on top.
-   [x] Setup completion always surfaces the auth prompt (when auth is enabled).
    -   Acceptance: after successful setup with auth enabled, the auth prompt is shown even though the API key is stored/active; dismiss hides it; no prompt forced when auth mode is `none`.
-   [x] Add a routable Health screen without adding it to sidebar nav.
    -   Acceptance: `/health` renders the health page, breadcrumb/title shows the Health label, and sidebar remains Home/Torrents/Settings only.
-   [x] Fetch and store `/health/full` and `/metrics` for the Health page.
    -   Acceptance: Health page shows basic + full health fields and metrics text when available; `/metrics` copy button works; errors surface as non-expiring toasts.
-   [ ] Remove vendored yewdux and anymap module once upstream is compatible with latest `yew`/`yew-router`.
    -   Acceptance: `vendor/yewdux` removed; workspace patch deleted; crates.io `yewdux` supports latest `yew`; ADR 074 closed.
    -   Status: blocked. crates.io `yewdux` 0.11.0 still depends on `yew` 0.21 (and `gloo` 0.10), so dropping the vendor would force us off `yew` 0.22.
    -   Evidence: `cargo tree -p yewdux` in a clean temp crate (no workspace patch) shows `yew v0.21.0` under `yewdux v0.11.0`.
-   [x] Implement SVG icon system with Yew components and IconButton.
    -   Acceptance: all inline SVG/icon spans replaced with icon components under `components/atoms/icons/*`, IconButton is used for icon-only actions, and hover/focus states align with DaisyUI patterns.
-   [x] Add Dashboard storage status dropdown actions (Enhance/Insights/Auto Tag/Delete).
    -   Acceptance: storage card has a Nexus-style dropdown with those exact actions; wired to callbacks or placeholder handlers as agreed.
-   [x] Align SSE indicator label with checklist (“Live” when connected).
    -   Acceptance: connected state displays “Live”; reconnecting/disconnected labels remain unchanged.
-   [x] Add category/tag management shortcuts to Add/Create torrent modals.
    -   Acceptance: Add/Create modals surface shortcuts linking to label management (or invoke label modal) per checklist.
-   [x] Update SSE list-row fields for tags/trackers.
    -   Acceptance: SSE updates can refresh tag/tracker fields in list rows without full refresh.
-   [x] Remove i18n fallback to default locale and raw key display.
    -   Acceptance: missing keys are surfaced explicitly (no default fallback strings), and English bundle covers all referenced keys.
-   [x] Remove git-sourced dependencies for UI crates.
    -   Acceptance: no git sources in `Cargo.lock`; `deny.toml` has no git allow-list; vendored yewdux tracked as a temporary exception.
-   [x] Raise workspace coverage to >=80% and verify `just ci`/`just cov` locally.
    -   Acceptance: `just cov` >= 80% line coverage; `just ci` passes without warnings.
    -   Priority targets (largest missed-line counts): `crates/revaer-config/src/loader.rs`, `crates/revaer-fsops/src/service/mod.rs`, `crates/revaer-torrent-libt/src/worker.rs`, `crates/revaer-app/src/orchestrator.rs`, `crates/revaer-app/src/bootstrap.rs`, `crates/revaer-data/src/config.rs`, `crates/revaer-cli/src/cli.rs`, `crates/revaer-ui/tools/asset_sync/src/lib.rs`.
    -   Current: `just cov` reports 80.44% line coverage and passes; `just ci` completed after latest changes.
-   [x] Allow `hashbrown`/`foldhash` multiple-version split to unblock `just lint` + `just deny`.
    -   Acceptance: `just lint` passes with `clippy::multiple_crate_versions` allowed and `cargo deny` permits duplicates; removal tracked in ADR 076.
    -   Current: `sqlx-core` pins `hashlink ^0.10.0` (hashbrown 0.15) while `yew` requires `indexmap ^2.11` (hashbrown 0.16).
-   [x] Replace undefined Nexus-style class usage with DaisyUI equivalents (or add a minimal CSS bridge).  
    -   Acceptance: Health + Labels views render using DaisyUI classes (card/badge/text utilities) or a documented, minimal CSS mapping; no undefined `panel`/`pill`/`muted`/`eyebrow`/`label-*` classes remain.
-   [x] Normalize Labels editor inputs to DaisyUI form controls.  
    -   Acceptance: Label editor inputs use `Input` component or `form-control` + `input` classes with consistent spacing/typography.
-   [x] Fix disk usage tabs to follow DaisyUI tab semantics.  
    -   Acceptance: tab elements are `button` with `role="tab"`, `aria-selected`, and `tab-active` state; stray `false` class removed (or tabs removed if they remain static placeholders).

## 0) Updates

-   [x] Update to `Yew` version 0.22. Use the following guides to help:
    -   https://yew.rs/docs/migration-guides/yew/from-0_18_0-to-0_19_0
    -   https://yew.rs/docs/migration-guides/yew/from-0_19_0-to-0_20_0
    -   https://yew.rs/docs/migration-guides/yew/from-0_20_0-to-0_21_0
    -   https://yew.rs/docs/migration-guides/yew/from-0_21_0-to-0_22_0
    -   Current: `yew` 0.22 with vendored `yewdux` (ADR 074).
-   [x] Upgrade to `Yewdux` version 0.11
-   [x] Upgrade to `Yew-Router` version 0.19
    -   Current: `yew-router` 0.19 aligned with `yew` 0.22 (vendored `yewdux`).
-   [x] Eliminate the vendored `anymap`
-   [x] Reevaluate the `gloo*` version and ensure we're on the latest compatible version (aligned to `yew` 0.22: `gloo` 0.11, `gloo-net` 0.5).
-   [x] Balance our versions while keeping `yew`/`yew-router` latest; vendored `yewdux` is a temporary exception (ADR 074).

## 1) UX constraints and source-of-truth rules

-   [x] Treat ui_vendor/nexus-html@3.1.0/src as markup reference only; do not run any Node tooling.
-   [x] Treat static/nexus as the runtime asset kit (compiled CSS, images, client-side JS).
-   [x] Use DaisyUI Blueprint MCP as the canonical reference for DaisyUI v5 component patterns, variants, accessibility, and form layout; if Nexus markup differs, Nexus wins for visual parity unless intentionally standardizing.
-   [x] Translation bundle surfaces missing keys (no default/fallback strings).
-   [x] Every i18n key referenced in the UI has an English value (no raw keys shown).
-   [x] Error surfaces never block navigation (overlays are non-blocking).
-   [x] All server errors render as persistent, dismissible toasts (no auto-timeout).

## 2) Theme + palette (from the logo)

-   [x] Establish a Revaer theme that leans into dark navy/purple with magenta-purple accent.
-   [x] Use token guidance from the logo: background ~#000030 to #000040; accent ~#6A0071; primary ~#901CBB; highlight ~#C42AC3.
-   [x] Apply tokens via DaisyUI theme variables or theme selection plus minimal overrides; keep contrast readable for table/list UIs.
-   [x] Tables use base-200 with base-100 hover; progress fills use primary tokens.
-   [x] Theme applied via `data-theme` on `<html>` and persisted to `localStorage`; default to dark.
-   [x] No hardcoded hex or Tailwind palette colors in components; use DaisyUI semantic tokens only.

## 3) Shell and routing (atomic composition)

-   [x] Implement AppShell template: sidebar + topbar + main outlet; preserve Nexus layout structure.
-   [x] Sidebar nav only Home, Torrents, Settings (no extra entries).
-   [x] Logs screen is routable via server menu (`/logs`) but not in the sidebar.
-   [x] Categories/Tags are not top-level routes; Health is routable but not in the sidebar; management lives in Settings tabs.
-   [x] Torrents route supports deep linking to selected torrent (open details drawer) via URL state.
-   [x] Static assets resolve under nested routes (Nexus CSS/icons load on /torrents and /settings).

## 4) Authentication and first-run flow

-   [x] On startup, determine configured vs not configured.
-   [x] If not configured: force Setup flow (blocking screen).
-   [x] If configured and auth missing: show auth prompt (blocking modal/screen).
-   [x] Setup flow: call admin/setup/start; handle "already configured" by switching to configured state.
-   [x] Setup flow: collect setup token and complete via admin/setup/complete.
-   [x] Setup completion returns API key (when auth enabled) and client begins using it immediately.
-   [x] After completion, route to auth prompt.
-   [x] Auth prompt (configured state): provide two tabs: API key, or Local auth (username/password).
-   [x] Store auth choice in local storage and attach to all API requests.
-   [x] API key maps to header x-revaer-api-key (per OpenAPI).
-   [x] Local auth sends Authorization: Basic ... (server may ignore).
-   [x] Settings allow "bypass local" toggle; when enabled default auth prompt to API key and avoid showing local auth first.
-   [x] Auth prompt is dismissible so navigation to Settings remains available.
-   [x] Auth prompt overlay is non-blocking (pointer events pass through to the shell).
-   [x] Setup prompt supports No Auth selection (local network).
-   [x] Anonymous auth mode omits auth headers for API calls.
-   [x] Persist API key + expiry in `localStorage`; clear on logout.
-   [x] Logout invalidates token server-side immediately.
-   [x] Auto-refresh API key before expiry (`/v1/auth/refresh`).
-   [x] Enforce 14-day token expiry policy end-to-end (server + client).
-   [x] No-auth mode works end-to-end without API key when configured.
-   [x] Logout button is present in the sidebar footer alongside the connectivity indicator.

## 5) State management and rendering performance (yewdux)

-   [x] Adopt yewdux for all shared UI/data state; avoid use_reducer + ContextProvider for shared data.
-   [x] Define a single normalized AppStore with domain sub-structs:
    -   [x] auth (configured, setup status, auth method, key/user presence, last auth error)
    -   [x] ui (theme, toasts, modal/drawer state, FAB open, busy flags)
    -   [x] torrents (normalized maps, filters, paging, selection, details cache)
    -   [x] labels (categories/tags caches)
    -   [x] health (basic/full snapshots)
    -   [x] system (system rates, SSE connection status)
-   [x] Normalize torrent data:
    -   [x] torrents.by_id: HashMap<Uuid, Rc<TorrentRowState>>
    -   [x] torrents.visible_ids: Vec<Uuid> (render list by IDs only)
    -   [x] torrents.selected: HashSet<Uuid> (bulk)
    -   [x] torrents.filters: TorrentsQueryModel (mirrors URL query)
    -   [x] torrents.paging: { cursor, next_cursor, limit, is_loading }
    -   [x] torrents.details_by_id: HashMap<Uuid, Rc<TorrentDetailState>> (optional cache; keep large vectors here, not in row state)
    -   [x] torrents.fsops_by_id: HashMap<Uuid, Rc<FsopsState>> (separate map; row derives a small badge slice)
-   [x] Implement selectors for row-level subscription:
    -   [x] select_visible_ids()
    -   [x] select_torrent_row(id) (for drawer)
    -   [x] select_torrent_progress_slice(id) (for list rows; minimal fields only)
    -   [x] select_is_selected(id) (for bulk checkbox)
    -   [x] select_system_rates() and select_sse_status()
-   [x] Ensure selector return values are cheap and stable:
    -   [x] Use Rc/Arc for row models and replace only the changed row pointer on updates.
    -   [x] Derive/implement PartialEq so unchanged slices do not trigger re-render.
    -   [x] Keep visible_ids as IDs; do not copy full row structs into list state.
-   [x] Row list components must subscribe only to progress/state slices, not full TorrentRowState, unless rendering the drawer.

## 6) API client layer (singleton + shared domain types)

-   [x] Implement a single ApiClient instance (created once) and share via a lightweight context ApiCtx.
-   [x] Enforce: no component constructs its own API client; all API calls go through the singleton.
-   [x] Restrict API calls to page controllers/services (effects/hooks/modules), not atoms/molecules.
-   [x] Atoms and molecules must never perform API calls or dispatch side effects.
-   [x] ApiClient must use existing domain types from shared backend crates (models, enums, request/response types); do not recreate parallel UI-only types.
-   [x] Prefer re-exporting or directly depending on shared crates for:
    -   [x] TorrentSummary, TorrentDetail, TorrentSettingsView, TorrentFile, TorrentLabelPolicy
    -   [x] EventEnvelope, Event, TorrentState, related enums
-   [x] Build only minimal transport/adaptation glue (headers, auth, pagination, SSE), not duplicate schemas.
-   [x] Endpoints: GET /health, GET /health/full.
-   [x] Endpoints: GET /metrics (optional viewer).
-   [x] Endpoints: GET /v1/torrents, POST /v1/torrents.
-   [x] Endpoints: GET /v1/torrents/{id}.
-   [x] Endpoints: POST /v1/torrents/{id}/action.
-   [x] Endpoints: PATCH /v1/torrents/{id}/options.
-   [x] Endpoints: POST /v1/torrents/{id}/select.
-   [x] Endpoints: GET /v1/torrents/categories; PUT /v1/torrents/categories/{name}.
-   [x] Endpoints: GET /v1/torrents/tags; PUT /v1/torrents/tags/{name}.
-   [x] Endpoints: POST /v1/torrents/create.
-   [x] Endpoints: GET /v1/torrents/events (SSE).
-   [x] Endpoints: POST /v1/auth/refresh.
-   [x] Endpoints: GET /v1/fs/browse.
-   [x] Endpoints: GET /v1/logs/stream (SSE).
-   [x] Endpoints: POST /admin/factory-reset.
-   [x] Centralize error parsing with ProblemDetails; display status/title/detail consistently.
-   [x] Implement rate limit handling (429) with user-visible backoff messaging and safe retry.
-   [x] API responses include proper CORS headers for the UI origin (including SSE).

## 7) SVG and icon system (reuse + consistency)

-   [x] Encapsulate every SVG as a Yew component under atoms/icons/\*.
-   [x] Icon component props: size, class, optional title, optional variant (outline/solid) when relevant.
-   [x] Build IconButton and standardize hover/active/focus states with DaisyUI Blueprint patterns.
-   [x] Replace all inline SVG usage in pages/components with the icon components.

## 8) Rust UI component library (atomic + props discipline)

-   [x] Build primitives: Buttons (variants, sizes, loading, icon slots), IconButton.
-   [x] Build primitives: Inputs (text, password, number), SearchInput with debounce.
-   [x] Build primitives: Select/MultiSelect, Checkbox/Toggle.
-   [x] Build primitives: Badge, Progress, Tooltip, Skeleton, EmptyState.
-   [x] Build primitives: Dropdown menu, Tabs.
-   [x] Build primitives: Modal, Drawer (details panel), Toast/Alert.
-   [x] Build primitives: Table/List row components and a sticky bulk action bar.
-   [x] Every component exposes all configurables as props: labels, counts, state, href, ids, optional sections, variants, and an extra class hook.

## 9) Torrents list page (main screen)

-   [x] Layout: Nexus dashboard styling, table-based view (DaisyUI table) with filter bar and FAB.
-   [x] Filter header matches Nexus orders layout (search + select row with right-side actions).
-   [x] Secondary filter row covers tags/tracker/extension + clear filters.
-   [x] Filters in URL query: query text (name), state, tags, tracker, extension.
-   [x] Pagination: limit and cursor; provide Load more using next cursor.
-   [x] Columns (TorrentSummary): name, state, progress, down/up rate, ratio, tags, trackers, updated timestamp.
-   [x] Sortable headers use Nexus sort affordances and update URL sort state.
-   [x] All torrent table values update live via SSE (state, progress, rates, ratio, size, ETA, tags, trackers, updated).
-   [x] Row click opens details drawer and updates route (deep link).
-   [x] Row menu actions: pause, resume, reannounce, recheck, sequential toggle.
-   [x] Row menu actions: set rate (download/upload bps).
-   [x] Row menu actions: remove (confirm + delete_data toggle).
-   [x] Bulk operations: multi-select checkboxes and bulk action bar.
-   [x] Bulk ops: issue /{id}/action per selected torrent in parallel with concurrency cap.
-   [x] Bulk ops: collect per-item failures, show summary toast, keep drawer closed unless single selection remains.
-   [x] Bulk actions: pause, resume, recheck, reannounce.
-   [x] Bulk actions: sequential on/off, rate set, remove (confirm + optional delete_data).
-   [x] Empty state uses DaisyUI empty state inside table/drawer only; bottom info banner removed.
-   [x] Legacy torrent list component removed (non-Nexus grid/list).

## 10) Torrent details drawer (tabs: Overview, Files, Options)

-   [x] Overview: summary fields + same actions; show last error if present.
-   [x] Files tab: render TorrentFile list; include/exclude and priority edits.
-   [x] Files tab: updates via POST /v1/torrents/{id}/select (support skip_fluff toggle if desired).
-   [x] Options tab: render TorrentSettingsView; only editable fields that map to PATCH /v1/torrents/{id}/options.
-   [x] Options tab: read-only settings shown as static rows (no fake toggles).
-   [x] Details caching discipline: keep large file vectors/settings off the hot row model; update drawer from details_by_id.
-   [x] Drawer empty state uses DaisyUI card styling (no custom placeholder overrides).
-   [x] Detail drawer layout uses DaisyUI tabs/tables/toggles (no legacy panel CSS).

## 11) FAB actions (Torrents screen)

-   [x] Add torrent modal supports magnet or metainfo_b64.
-   [x] Add torrent modal always generates id client-side (UUID v4).
-   [x] Add torrent modal allows initial tags/category and initial rate limits if supported.
-   [x] Create torrent modal wired to POST /v1/torrents/create.
-   [x] Create torrent modal provides copy buttons for magnet/metainfo.
-   [x] Provide shortcuts to manage categories and tags.
-   [x] Add/Create torrent modals use DaisyUI form controls (no legacy panel CSS).

## 12) Categories and Tags pages (policy management)

-   [x] Categories list and editor: list existing, create/update via PUT with TorrentLabelPolicy fields.
-   [x] Categories editor: structured form with Advanced section for rarely used policy fields.
-   [x] Tags list and editor: same pattern as categories.

## 13) Health page (operator-facing)

-   [x] Show /health basic status.
-   [x] Show /health/full: degraded components, version/build info, and any torrent snapshot fields.
-   [x] Optional: /metrics viewer with copy button.

## 14) Live updates over SSE (robust, header-capable, envelope-first)

-   [x] Implement SSE using fetch streaming and manual SSE parsing (EventSource cannot set headers).
-   [x] Attach x-revaer-api-key to SSE requests.
-   [x] Endpoint discovery: primary /v1/torrents/events.
-   [x] Fallback: attempt /v1/events/stream if primary 404s.
-   [x] Parse SSE fields: event, id, retry, data.
-   [x] Treat the canonical payload as EventEnvelope { id, timestamp, event } JSON; decode this first.
-   [x] Back-compat decode: if envelope parse fails, attempt { kind, data } dummy payload mapping to an internal envelope shape.
-   [x] All SSE events must be normalized into a single internal EventEnvelope shape and applied through a single reducer path.
-   [x] Persist and send Last-Event-ID header for replay; store last numeric event id after successful envelope decode.
-   [x] Build SSE query filters from UI state:
    -   [x] torrent = comma-separated visible IDs when count is below a safe cap; otherwise omit.
    -   [x] event = kinds relevant to current view (list vs drawer).
    -   [x] state only when a state filter is active.
-   [x] SSE event filter names match backend enum (no 400 warnings).
-   [x] Handle 409/conflict SSE failures by resetting Last-Event-ID and reconnecting cleanly.
-   [x] Implement a progress coalescer:
    -   [x] Buffer incoming progress patches per torrent ID in a non-reactive buffer.
    -   [x] Flush buffered progress into yewdux store on a fixed cadence (50-100ms).
    -   [x] Apply flush by replacing only the affected Rc<TorrentRowState> entries.
    -   [x] Apply non-progress events immediately (add/remove/state change/metadata/fsops/files/selection/system/health).
    -   [x] If an event cannot be decoded/applied, trigger a throttled targeted refresh (do not refetch everything per message).
-   [x] Progress coalescing is mandatory to cap render frequency under high event volume.
-   [x] SSE status includes connected/reconnecting/disconnected and sidebar indicator reflects all states.
-   [x] UI: show "Live" indicator when connected and subtle warning when reconnecting/backing off.

## 15) Not in scope guardrails

-   [x] Do not implement qBittorrent compatibility endpoints.
-   [x] Do not add node/npm/vite/tailwind build steps.
-   [x] Do not add fake UI controls for settings that cannot be persisted via existing endpoints.

## 16) Hardline dashboard rebuild (Nexus + DaisyUI)

-   [x] Inventory old dashboard entrypoint/shell/components/styles for cleanup.
-   [x] Nexus source-of-truth files identified (dashboard + sidebar/topbar + storage status partials).
-   [x] Sidebar nav = Home, Torrents, Settings (only) with SSE indicator pinned at the bottom.
-   [x] Sidebar defaults to icon-only (labels only when expanded).
-   [x] Sidebar labels show Home/Torrents/Settings across locales (nav labels standardized).
-   [x] Sidebar footer SSE indicator uses Nexus pinned-footer structure and DaisyUI primitives.
-   [x] Sidebar SSE indicator label expands only when the sidebar is expanded.
-   [x] Sidebar footer includes Logout in the same bar; label collapses with the sidebar.
-   [x] Topbar is consistent with breadcrumb, theme toggle, language selector, server menu.
-   [x] Topbar location indicator matches Home/Torrents/Settings consistently.
-   [x] Server menu items exactly: Restart server, View logs; Factory reset separated and styled danger.
-   [x] Server dropdown renders above torrent action bar (z-index stacking).
-   [x] Home dashboard matches Nexus layout with DaisyUI cards/stats/progress/list.
-   [x] Dashboard storage status dropdown uses Nexus actions (Enhance/Insights/Auto Tag/Delete).
-   [x] SSE indicator modal is non-blocking; navigation always reachable.
-   [x] SSE indicator modal shows status, next retry time, last event ID, last error reason, retry strategy, and Retry/Dismiss actions.
-   [x] Settings is sectioned and reachable without auth; test connection + config snapshot wired.
-   [x] Legacy dashboard CSS reduced; Nexus app.css remains primary styling.
-   [x] Task record (ADR) added for this work.
-   [x] `just ci` passes locally.
    -   `just ci` completed successfully after lint/deny exceptions (Finding 15).
-   [x] `just cov` meets the ≥80% line coverage gate.
    -   Current: 80.44% total line coverage.

## 17) Settings UX (tabs + batching + file browser)

-   [x] Settings tabs map to torrent-user workflows (Connection, Downloads, Seeding, Network, Storage, Labels, System).
-   [x] Changes are staged locally with a change-count badge and a single Save action.
-   [x] Directory fields use a remote file browser modal with browse + manual path entry.
-   [x] Server validates selected directory exists before accepting updates; surface failures via persistent toasts.
-   [x] Settings API/DB is fully normalized (no JSON blobs); UI renders typed fields.
-   [x] Boolean fields use toggles; fixed options use dropdowns; numeric values use number inputs.
-   [x] Read-only fields show text with copy-to-clipboard controls.
-   [x] Settings errors are shown inline via alerts/toasts; no blocking overlays.

## 18) Logs screen + factory reset

-   [x] Logs screen uses SSE stream only while mounted; connection closes on leave.
-   [x] Log view honors ANSI color codes and extended characters; log body is black.
-   [x] Log list is bounded (memory efficient) and prepends new lines; only log area scrolls.
-   [x] Factory reset modal requires typing `factory reset` (client + server validation).
-   [x] Factory reset errors show raw server detail in non-expiring toasts; success reloads to setup.
