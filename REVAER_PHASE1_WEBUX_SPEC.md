# Revaer Web UI — Phase 1 Design Specification

A complete design specification for the Revaer Web UI, based on the finalized questionnaire responses.
Scope: Torrent‑management‑only (Phase 1), with future expansion paths documented.

---

# 1. Product Framing & Constraints

## 1.1 Vision

Revaer Web UI is a **clean, modern home‑server UI** for managing torrents with **Arr‑level capabilities**, designed with **power‑user controls** but a **simple UX-first layout**.
It replaces qBittorrent/Deluge/Sonarr/Radarr-style interactions with a unified, high-performing, browser-based interface.

## 1.2 Priorities

1. **Simplicity**
2. **Aesthetic polish**
3. **Power features**

## 1.3 Target Users

-   Broad open-source user base
-   Expected mix of technical and semi-technical users
-   Must remain intuitive out-of-the-box, but customizable

## 1.4 Access Model

-   **Local + secure remote access**
-   **Remote-first (VPN / reverse proxy / tunnel aware)**

Design must accommodate:

-   TLS termination awareness
-   Proxy header awareness
-   Color-coded remote status indicators
-   Graceful failure when behind Cloudflare Tunnels, Tailscale, ZeroTier, etc.
-   Auth UX: API key prompt stored in local storage; remote mode always requires key. If backend advertises `allow_anonymous`, LAN mode can skip the prompt; otherwise key is required.

## 1.5 Phase 1 Scope

-   Torrent management only
-   Indexers, library mgmt, and media-aware grouping intentionally _out of scope_ until Phase 2

---

# 2. Tech Stack & Repository Structure

## 2.1 Frontend Framework

-   **Yew** (Rust → WASM)
-   Targets fast boot, small bundle size, native Rust type reuse
-   Lazy-loaded for snappy UI
-   Skeletons for lazy-loaded elements/data

## 2.2 Repository Structure

Monorepo layout:

```
/crates
  /revaer-api
  /revaer-app
  /...
  /revaer-ui   ← this spec
  /revaer-runtime
  /revaer-telemetry
  /...
/docs
```

## 2.3 API Style

-   REST for structured data (auth via API key header; no cookies). Proposed endpoints:
    -   `GET /v1/torrents?search=&status=&tag=&tracker=&path=&regex=` (paginated, sortable)
    -   `POST /v1/torrents` (magnet/url/file upload)
    -   `GET /v1/torrents/{id}`
    -   `PATCH /v1/torrents/{id}` (pause/resume/recheck/priorities/wanted)
    -   `PATCH /v1/torrents/{id}/location`
    -   `PATCH /v1/torrents/{id}/tags`
    -   `DELETE /v1/torrents/{id}?with_data=bool`
    -   `GET /v1/dashboard`
    -   `GET /v1/jobs`
    -   `GET /v1/settings`
    -   `PATCH /v1/settings` (admin only)
-   SSE for:
    -   Torrent updates
    -   Transfer rate telemetry
    -   Queue updates
    -   Jobs/post-processing updates
    -   VPN state
-   SSE payload shape: `{ "kind": "<event_kind>", "data": { ... } }` with typed `kind` values (`torrent_progress`, `torrent_state`, `torrent_rates`, `torrent_added`, `torrent_removed`, `jobs_update`, `vpn_state`, `system_rates`, `queue_status`).
-   Pagination/sort params: `page`, `per_page`, `sort`, `dir=asc|desc`; response wraps `{ data: [...], meta: { page, per_page, total, sort: { col, dir } } }`.
-   Core payloads:
    -   Torrent: `{ id, name, status, progress, eta_seconds, ratio, size_bytes, downloaded_bytes, uploaded_bytes, download_bps, upload_bps, category, tags, tracker, save_path, added_at, completed_at }`
    -   Files: `{ path, size_bytes, completed_bytes, priority, wanted }`
    -   Peers: `{ ip, client, flags, country, download_bps, upload_bps, progress }`
    -   Trackers: `{ url, status, next_announce_at, last_error, last_error_at }`
    -   SSE payloads: `torrent_progress { torrent_id, progress, eta_seconds, download_bps, upload_bps }`, `torrent_state { torrent_id, status, reason }`, `torrent_rates { torrent_id, download_bps, upload_bps }`, `torrent_added/removed { torrent_id }`, `jobs_update { jobs: [{ id, torrent_id, kind, status, detail, updated_at }] }`, `vpn_state { state, message, last_change }`, `system_rates { download_bps, upload_bps }`, `queue_status { active, paused, queued, depth }`

## 2.4 Authentication Model

Hybrid:

-   **LAN mode:** optional zero-auth
-   **Remote mode:** multi-user with roles (Admin, User, Read-only)
-   **Remote mode always enforces auth**

---

# 3. Core Feature Set

## 3.1 Dashboard Requirements

Dashboard displays:

-   Global upload speed
-   Global download speed
-   Active / Paused / Completed counts
-   Disk usage
    -   Global
    -   Per-path
-   Recent events
-   Tracker health summary
-   Queue status
-   VPN status (connected, routing, error, off)

Data flow:

-   Static values via REST
-   Dynamic values via SSE stream
-   Data definitions:
    -   Ratio: upload/download, clamp to 2 decimals; zero-download yields `0.0`
    -   ETA: `remaining_bytes / rate` when rate > 0, else `null`
    -   Rates: bytes/sec integers; render using binary units in UI
    -   Tracker health: aggregate tracker states into ok/warn/error counts
    -   VPN status: enum `connected | routing | error | off` with message + last_change
    -   Queue status: active/paused/queued counts and queue depth

## 3.2 Torrent List

Columns required:

-   Name
-   Status
-   Progress
-   ETA
-   Ratio
-   Tags / labels
-   Tracker
-   Save path
-   Category / media type
-   File size
-   Upload speed
-   Download speed

Functional requirements:

-   Sortable columns
-   Pinned columns
-   Virtualized table for **50k+ torrents**
-   Row density toggle (compact / normal / comfy)
-   Sorting: server-driven; default `added_at desc`; preserve per-view state
-   Filters: AND across fields, OR within multi-select for a field; regex opt-in (`regex=true`) applies to name/path
-   Column collapse priority (keep longest): Name, Status, Progress, Down/Up speed, ETA, Ratio, Size, Category/Tags, Tracker, Save path

## 3.3 Torrent Detail View

Sections:

### **Files**

-   Tree view
-   Per-file priority
-   File progress
-   Wanted/unwanted toggle

### **Peers**

-   IP
-   Speeds
-   Flags
-   Client type
-   Country (GeoIP optional)

### **Trackers**

-   Announce URL
-   Status
-   Next announce
-   Errors

### **Event Log**

-   Time-stamped torrent events
-   Warnings
-   Tracker issues
-   State transitions

### **Metadata**

-   Hash
-   Magnet
-   Size
-   Piece count
-   Piece size
-   Behaviors:
    -   File tree: per-file priority + wanted/unwanted toggles; batch PATCH on blur/explicit save with optimistic UI and rollback on error
    -   Peers: columns IP, client, flags, country (if available), up/down speed, progress; sortable by speed/progress
    -   Trackers: status, next announce, last error string/time; warning badge on failures

## 3.4 Adding Torrents

Supported methods:

-   Upload `.torrent` file
-   Magnet link
-   Paste URL
-   Auto-detect from file drop
-   Local watch folder (configured in Settings)
-   UX details:
    -   Drag/drop zone accepts .torrent files and magnet text; validate magnet/URL with inline errors
    -   Pre-submit fields (if backend supports): category/tags/save path; otherwise use defaults
    -   Watch folder: show current path, enabled/disabled, last scan time; surface errors in Jobs/Post-processing list
    -   Post-submit: toast plus option to open detail drawer
    -   Error copy: “Add failed: <reason>”; invalid input errors inline (“Invalid magnet link”, “Unsupported file type”)

## 3.5 Search & Filtering

Supports:

-   Basic text search
-   Multi-filter (status, tags, trackers, paths)
-   Regex mode
-   Saved smart views (Phase 2)

## 3.6 Bulk Operations

-   Pause / Resume
-   Recheck
-   Delete
-   Delete + data
-   Change location
-   Change category/tags
-   Set priority

## 3.7 Out-of-Scope (Phase 1)

-   Indexer management
-   Media-aware grouping
-   Season/movie linking
-   External metadata loading

---

# 4. Information Architecture & Navigation

## 4.1 Top-Level Navigation

-   **Dashboard**
-   **Torrents** (primary working view)
-   **Search**
-   **Indexers** (hidden until Phase 2)
-   **Library** (hidden until Phase 2)
-   **Jobs / Post-processing**
-   **Settings**
-   **Logs**

## 4.2 UI Modes

-   **Simple mode:** minimal exposed controls
-   **Advanced mode:** full power-user surface

Mode switching does not reload the app.

-   Persist selection in local storage; default to Simple on first run. Advanced reveals full filters/columns/bulk ops.

## 4.3 Responsiveness & Mobile‑First Design

Revaer Web UI must follow a **mobile‑first, responsive layout strategy**. The layout must gracefully scale from **360px mobile** through **4K desktop**.

### **4.3.1 Breakpoints**

Engineers must implement the following official breakpoints:

-   **xs: 0–479px** (small mobile)
-   **sm: 480–767px** (mobile landscape / small tablets)
-   **md: 768–1023px** (tablets)
-   **lg: 1024–1439px** (laptops)
-   **xl: 1440–1919px** (desktop)
-   **2xl: 1920px+** (large desktop / ultrawide)

Breakpoints should use **min-width media queries**.

### **4.3.2 Layout Behavior by Breakpoint**

#### **Mobile (xs–sm)**

-   Torrent list uses **card view**, one torrent per card.
-   Key stats appear in a **two‑column grid** inside the card.
-   Long torrent names truncate with ellipsis.
-   Actions appear as a **bottom sheet** or **floating action row**:

    -   Pause / Resume
    -   Delete
    -   More (…)

-   Search bar spans full width at top.
-   Navigation collapses into a **hamburger menu** with slide‑out drawer.
-   Dashboard widgets stack vertically in cards.

#### **Tablet (md)**

-   Torrent list switches to a **compact table**, 2–4 visible columns.
-   File tree and metadata tabs stack under a unified header.
-   Dashboard uses a **two‑column responsive grid**.
-   Sidebar navigation becomes optional — collapsible.

#### **Laptop (lg)**

-   Full table with all default columns visible.
-   Sidebar becomes fixed on the left with icons+labels.
-   Dashboard uses a **three‑column grid** where possible.

#### **Desktop / Ultrawide (xl–2xl)**

-   Torrent table may use **adaptive column expansion**.
-   Metadata and file tree can be displayed in a **split‑pane layout**.
-   Dashboard uses a **four‑column grid** with larger stat tiles.
-   Ensure readable max‑width for text content (no >150ch lines).

### **4.3.3 Table Responsiveness Rules**

-   Columns must be **priority‑ranked** and collapse when space is limited.
-   Required columns:
    -   Name
    -   Status
    -   Progress
    -   Speed up/down
-   Non‑critical columns move to:
    -   Expandable row
    -   Details drawer
    -   “More…” overflow menu
-   Table virtualization must support horizontal scrolling on small displays _without breaking keyboard navigation_; use roving tabindex and preserve horizontal scroll at xs/sm.

### **4.3.4 Component Responsiveness Requirements**

#### **Dashboard Widgets**

-   Must support dynamic resizing.
-   Cards rearranged using CSS grid auto‑flow.
-   No hardcoded pixel‑width elements.

#### **Torrent Detail View**

-   On mobile:
    -   Converts to **tabbed interface** (`Files`, `Peers`, `Trackers`, `Log`, `Info`)
-   On desktop:
    -   Uses **horizontal split layout** enabling simultaneous visibility of:
        -   File tree
        -   Metadata
        -   Peers list

#### **File Tree**

-   Converts to an **accordion list** on mobile.
-   Full tree‑view only enabled at md+.

#### **Navigation**

-   Mobile: Hamburger → slide‑out drawer.
-   Tablet+: Collapsible sidebar.
-   Desktop+: Fixed full sidebar with labels + icons.

### **4.3.5 Interaction Patterns**

-   All interactive elements must maintain **40px minimum touch target** on mobile.
-   Swipe gestures optional (Phase 2).
-   Buttons scale using CSS variables for touch vs pointer devices.
-   Confirmation copy:
    -   Delete: “Remove torrent ‘<name>’? Files remain on disk.” Actions: Cancel / Remove.
    -   Delete + data (shift+delete or checkbox): “Remove torrent and delete data? This cannot be undone.” Actions: Cancel / Delete data.
    -   Recheck: “Recheck data for ‘<name>’?” Actions: Cancel / Recheck.
    -   Watch-folder errors surface in Jobs/Post-processing: “Watch folder scan failed: <reason>”.

### **4.3.6 Performance Requirements**

-   Initial UI load under **300ms** on modern mobile devices.
-   Table virtualization mandatory for all views beyond 500 rows.
-   SSE event batching required to reduce layout thrashing.
-   Keep main-thread work bounded for 50k rows; measure with a Lighthouse-like check in CI.

### **4.3.7 Accessibility on Mobile**

-   Keyboard navigation must not break when layout collapses.
-   Screen‑reader flow must follow DOM order, not visual order.
-   Focus traps for drawers and modals must be enforced.

## 4.4 Multi-Instance Support

-   Not in Phase 1
-   Reserved navigation slot for Phase 3

---

# 5. Theming

## 5.1 Scope

-   Light + Dark themes only (Phase 1)
-   Token-based expansions possible
-   Token set: spacing (4/8/12/16/24/32), radius (4/8/12), elevation tiers (flat/raised/floating), typography scale (xs–2xl with consistent line heights).
-   Component states: hover/focus/active/disabled tokens; focus ring 2px `accent-500`/`accent-dark-500`; pressed state darkens by one tone; inputs/tables use border tokens.
-   Typography: prefer expressive but readable stack (e.g., “Inter, 'Segoe UI', system-ui”) unless a brand typeface is provided.
-   Charts: lightweight WASM-friendly library (e.g., plotters) with small sparklines for rates.

## 5.2 OS Preference

-   Defaults to user OS
-   User override persists in local storage

---

## 5.3 Brand Palette

The Revaer UI uses a dual-theme color system:

-   **Revaer Dark** — primary, default theme for desktop UI (matches the dashboard reference image).
-   **Revaer Light** — complementary, future-ready light mode using the same brand hues.

All tokens below must be represented in the Tailwind + daisyUI theme configuration. Names are descriptive to aid mapping, but engineers may adapt to `primary`, `secondary`, `base-100`, etc., as long as semantics are preserved.

---

### 5.3.1 Revaer Dark Theme Palette

This theme matches the attached dark dashboard mock: deep blue/indigo surfaces, neon magenta/violet brand, cyan accents.

#### Brand & Accent Colors

Brand gradient (used for logo, high-impact accents):

-   `brand-gradient-start` – **#F43F9E** (neon magenta)
-   `brand-gradient-mid` – **#A855F7** (electric violet)
-   `brand-gradient-end` – **#6366F1** (indigo blue)

Solid brand tokens:

-   `primary` – **#A855F7**
    Primary actions, active nav text, primary badges.
-   `primary-soft` – **#7C3AED**
    Hover/pressed state for primary, lower-intensity fills.
-   `secondary` – **#6366F1**
    Secondary buttons, secondary data highlights.
-   `accent` – **#22D3EE**
    Links, subtle accents, focus rings.

These are the only saturated hues intended for frequent use; all other UI areas should lean on neutrals.

#### Surfaces & Backgrounds

Layered surfaces support depth without overwhelming contrast:

-   `bg-app` – **#050816**
    Root application background (body).
-   `bg-sidebar` – **#050B16**
    Sidebar base surface.
-   `bg-surface-1` – **#0B1020**
    Primary cards (stats, metrics, VPN card).
-   `bg-surface-2` – **#111827**
    Raised cards (Recent Events, Tracker Health, Queue Status).
-   `bg-surface-3` – **#1F2933**
    Highest elevation surfaces (modals, toasts) — use sparingly.
-   `bg-sidebar-active` – **#1E1B4B**
    Base for active nav item background (with optional low-opacity gradient overlay).
-   `bg-table-header` – **#0F172A**
-   `bg-table-row` – **#020617**
-   `bg-table-row-alt` – **#02091A**

In daisyUI terms:

-   Map `base-100` → `bg-surface-1`
-   Map `base-200` → `bg-surface-2`
-   Map `base-300` → `bg-surface-3`

#### Text Colors

-   `text-primary` – **#E5E7EB**
    Main content text (card titles, table rows).
-   `text-secondary` – **#9CA3AF**
    Subtitles, helper copy, column headers.
-   `text-muted` – **#6B7280**
    Hints, placeholders.
-   `text-disabled` – **#4B5563**
    Disabled labels, inactive controls.
-   `text-inverse` – **#020617**
    Text on bright badges (warning/success) or chips.

daisyUI mapping suggestion:

-   `--fallback-bc` / `base-content` → `text-primary`
-   `neutral-content` → `text-secondary`

#### Borders, Dividers & Outlines

-   `border-subtle` – **#1F2933**
    Card/table borders, subtle separators.
-   `border-strong` – **#374151**
    Major section dividers.
-   `divider` – **#111827**
    Thin rules (e.g., table header underline).
-   `focus-ring` – **#22D3EE**
    2px focus outline for keyboard navigation.

#### Semantic Status Colors

Used consistently for torrent and system states:

-   `info` – **#38BDF8**
    Neutral informational states.
-   `success` – **#22C55E**
    Healthy torrents (seeding), OK tracker state.
-   `warning` – **#EAB308**
    Paused, stalled, or attention-needed state.
-   `error` – **#F97373**
    Failed torrents, tracker error, system error.
-   `neutral-pill` – **#6B7280**
    Non-critical neutral labels (e.g., "Queued").

These must be wired to daisyUI `info`, `success`, `warning`, and `error` tokens.

#### Progress, Charts & Micro-Visuals

-   `progress-bg` – **#1F2933**
    Background track for progress bars.
-   `progress-primary` – **#A855F7**
    Default progress fill.
-   `progress-secondary` – **#6366F1**
    Used for secondary metrics.
-   `queue-bar` – **#A855F7**
    Vertical bars in Queue Status card.
-   `queue-bar-muted` – **#4C1D95**
    Less-active queue states.

#### Sidebar & Navigation Specifics

-   `nav-text` – **#CBD5F5**
    Primary nav text.
-   `nav-text-muted` – **#6B7280**
    Non-active nav items.
-   `nav-icon` – **#9CA3AF**
    Default icon color.
-   `nav-icon-active` – gradient from **#F43F9E** → **#A855F7**
    Applied via gradient fill or mask.

Active nav item background:

-   `nav-active-bg` – **#1E1B4B** with optional subtle brand gradient overlay on the left edge.

---

### 5.3.2 Revaer Light Theme Palette

The light theme is visually complementary, not a separate brand. All brand hues remain identical; only neutrals and surfaces invert.

#### Brand & Accent Colors (shared)

-   `primary` – **#A855F7**
-   `primary-soft` – **#8B5CF6**
-   `secondary` – **#6366F1**
-   `accent` – **#0EA5E9**
    Slightly lighter teal for better contrast on light backgrounds.

Logo gradient should reuse `brand-gradient-start/mid/end` on a light‑friendly backdrop.

#### Surfaces & Backgrounds

-   `bg-app` – **#F3F4F6**
    Overall page background.
-   `bg-sidebar` – **#F9FAFB**
    Sidebar background.
-   `bg-surface-1` – **#FFFFFF**
    Main cards.
-   `bg-surface-2` – **#E5E7EB**
    Raised surfaces, hovered table rows.
-   `bg-surface-3` – **#D1D5DB**
    Highest elevation elements (modals, toasts).
-   `bg-sidebar-active` – **#E0E7FF**
    Active nav item background.
-   `bg-table-header` – **#F3F4F6**
-   `bg-table-row` – **#FFFFFF**
-   `bg-table-row-alt` – **#F9FAFB**

Suggested daisyUI mapping:

-   `base-100` → `bg-surface-1`
-   `base-200` → `bg-surface-2`
-   `base-300` → `bg-surface-3`

#### Text Colors

-   `text-primary` – **#111827**
    Core content text.
-   `text-secondary` – **#4B5563**
    Secondary labels.
-   `text-muted` – **#9CA3AF**
    Helper copy and placeholders.
-   `text-disabled` – **#D1D5DB**
    Disabled text.
-   `text-inverse` – **#F9FAFB**
    Text on primary/secondary buttons and strong badges.

#### Borders & Dividers

-   `border-subtle` – **#E5E7EB**
    Card/table borders.
-   `border-strong` – **#D1D5DB**
    Section dividers.
-   `divider` – **#E5E7EB**
-   `focus-ring` – **#6366F1**
    Indigo focus outline.

#### Semantic Status Colors (Light)

Use the same semantics as dark theme, adjusted for light contrast:

-   `info` – **#0EA5E9**
-   `success` – **#16A34A**
-   `warning` – **#F59E0B**
-   `error` – **#EF4444**

All semantic badges should use `text-inverse` to maintain readability.

#### Progress, Charts & Elements

-   `progress-bg` – **#E5E7EB**
-   `progress-primary` – **#A855F7**
-   `progress-secondary` – \*\*#6366F1`
-   `queue-bar` – \*\*#6366F1`
-   `queue-bar-muted` – \*\*#C4B5FD`

#### Sidebar & Navigation (Light)

-   `nav-text` – **#111827**
-   `nav-text-muted` – **#6B7280**
-   `nav-icon` – **#6B7280**
-   `nav-icon-active` – `primary` (violet)

Active nav item background:

-   `nav-active-bg` – **#E0E7FF** with optional very subtle left‑edge brand gradient.

---

### 5.3.3 Usage Notes

-   All components must derive their colors from these tokens via Tailwind/daisyUI theme configuration. No ad‑hoc hex values should be used in markup.
-   The dark theme is the default for Phase 1. The light theme is defined now so that engineers can wire both themes via daisyUI without revisiting color decisions later.
-   Any additional colors introduced (e.g., for charts) must be sampled from or derived from this palette to maintain visual cohesion.

# 6. Localization & i18n

## 6.1 Initial Languages

Must support:

```
ar, de, es, hi, it, jv, mr, pt, ta, tr,
bn, en, fr, id, ja, ko, pa, ru, te, zh
```

## 6.2 RTL

-   **Full RTL support required in v1**
    This includes:
-   Bi-directional layout
-   Mirrored progress bars
-   Reversed file trees
-   RTL-aware table alignment

## 6.3 Date/Number Formatting

-   Hybrid model:
    -   Browser locale by default
    -   User-selectable override
    -   Safe fallback to English

## 6.4 Translation Format

-   Local JSON files stored in:

```
/revaer-ui/i18n/*.json
```

-   Fallback: browser locale → English; missing keys fall back to English string.
-   Pluralization: ICU-like keys per locale (e.g., `torrent_count.one`, `torrent_count.other`); torrent states localized via string table.
-   Numbers/dates: Intl APIs or polyfill; binary units for sizes; localized date/time formats.

## 6.5 Special Rules

-   Pluralization rules
-   Torrent-state linguistic variants
-   Locale-aware data units (GB vs GiB)

---

# 7. Accessibility & UX

## 7.1 Accessibility Standard

-   Must meet **WCAG 2.1 AA**

Includes:

-   Keyboard navigation
-   Focus states
-   Semantic markup
-   High-contrast dark mode
-   Descriptive ARIA for torrent rows, progress, tags

## 7.2 Keyboard Shortcuts

Enabled:

-   `/` — Search
-   `j/k` — Move selected row
-   `space` — Pause/Resume
-   `delete` — Delete prompt
-   `shift + delete` — Delete + data
-   `p` — Recheck

## 7.3 Notification Model

-   Toasts/snackbars
-   Persistent activity panel

## 7.4 Error Verbosity

-   Collapsed by default
-   Expand for full technical detail

---

# 8. Integration With Revaer System

## 8.1 Indexers

-   Hidden in Phase 1
-   Will surface once Tubarr / indexer backend stabilizes

## 8.2 Post-Processing

-   Show job states
-   Show failures
-   No editing or workflow controls yet

## 8.3 Notifications

-   None in Phase 1
-   Infrastructure hooks reserved

## 8.4 User Roles

-   Admin
-   User
-   Read-only
-   Permissions:
    -   Read-only: GET-only
    -   User: add/pause/resume/recheck; no delete-data or settings edits
    -   Admin: all operations including delete + data and settings mutations

---

# 9. Non-Functional Requirements

## 9.1 Torrent Count Capacity

-   **50k+ torrents**
-   Virtualized tables required
-   All operations async-streamed

## 9.2 Real-Time Update Frequency

-   Sub-second via **SSE**
-   Backoff strategy required for:
    -   High-load
    -   Mobile
    -   Restricted networks

## 9.3 Backend Downtime UX

-   Graceful reconnect overlay
-   Retry countdown
-   Diagnostics panel with:
    -   Reason
    -   Last event
    -   SSE status
    -   Network mode
-   Reconnect behavior: exponential backoff with jitter (1s → 30s), overlay with last-event timestamp and retry countdown; toast for transient errors.

## 9.4 Log Console

-   Developer-only toggle
-   Hidden by default
-   Streams engine logs in real time

---

# 10. Documentation & Design System

## 10.1 Location

-   `docs/ui/`
-   Include navigation flow (Mermaid), component graph, SSE event flow, torrent lifecycle diagrams, documented tokens (colors, type, spacing), breakpoints, and an accessibility checklist.
-   Component showcase served via `trunk`/`wasm-pack` preview (Storybook-like).

## 10.2 Mermaid Diagrams

Required:

-   Navigation flow
-   Component graph
-   SSE event flow
-   Torrent lifecycle

## 10.3 Component Library

-   Revaer UI uses a **dedicated design system**:
    -   Buttons
    -   Tables
    -   Switches
    -   Inputs
    -   Toggles
    -   Charts
    -   File trees
    -   Progress bars

## 10.4 Testing

-   Playwright E2E
-   Unit tests for:
    -   Table virtualization
    -   SSE handlers
    -   Torrent parsers
-   Storybook-like “component showcase” for contributors
-   Playwright scenarios: login/API key flow, dashboard metrics load, torrent list sort/filter, add magnet, pause/resume, delete + confirm (and delete + data via shift+delete path), detail tabs (files/peers/trackers/log), SSE disconnect/reconnect overlay, RTL toggle, dark mode toggle, mobile breakpoint layout, recheck shortcut (`p`).
-   Performance budget: verify <300ms initial load on cached assets using Lighthouse Mobile preset (Moto G4/Slow 4G) on second load; allow ±10% variance in CI. Ensure virtualization keeps main-thread work bounded for 50k rows and Total Blocking Time <150ms during scroll simulation.

---

# End of Phase 1 Specification

# Revaer UI — UX Engineering Specification (Phase 1)

**Version:** 1.0
**Owner:** UX Engineering
**Audience:** UI Engineers, Yew Developers, QA, Product
**Stack:** Yew + TailwindCSS + daisyUI
**Scope:** Desktop‑first responsive dashboard, left‑sidebar navigation, metrics display, routing scaffolding
**Non‑scope:** API implementation details, data virtualization implementation, business logic

---

## 1. Product & UX Direction

Revaer is a **clean, modern home‑server UI** for managing torrents with **Arr‑level capabilities**, designed with **power‑user controls** but a **simple, discoverable layout**. The UI should feel like:

-   A focused, dark, neon‑noir dashboard
-   Built for local server operators and power users
-   Approachable enough for semi‑technical household users

The visual direction is:

-   Dark background, high contrast
-   Neon magenta/violet brand accent
-   Soft cards, clear hierarchy
-   Minimal decorative chrome, maximum signal

The experience must:

-   Be fast and responsive
-   Avoid clutter and over‑nested navigation
-   Emphasize clarity of torrent state, performance, and health

---

## 2. Technical Stack & UI Frameworks

### 2.1 Frontend Framework

-   **Yew (Rust → WASM)** as the UI framework
-   Component‑based architecture with:
    -   App‑level shell component
    -   Route‑level page components
    -   Reusable UI primitives (cards, tables, badges, etc.)

### 2.2 Styling & Components

-   **TailwindCSS** for utility‑first styling
-   **daisyUI** as the primary component framework on top of Tailwind
-   **Custom Revaer daisyUI theme** (named `revaer`) is required
-   No standalone `.css` files for layouts; layout is expressed via Tailwind utility classes and daisyUI component classes.

### 2.3 DaisyUI Theme Requirements

Define a custom theme `revaer` with at least these tokens:

-   `primary`: neon magenta/violet (for brand and primary actions)
-   `secondary`: deep blue/purple (for secondary emphasis)
-   `accent`: cyan/blue (for links, subtle accents)
-   `neutral`: slate/graphite tones for sidebar and text
-   `base-100`, `base-200`, `base-300`: layered dark backgrounds for surface, raised surface, and higher elevation cards
-   `info`, `success`, `warning`, `error`: mapped to torrent‑related statuses (info, seeding, paused, error)

Only the `revaer` theme (and optionally a `revaer-light` in the future) should be enabled. All default daisyUI themes must be disabled to avoid visual drift and bloated CSS.

### 2.4 Tailwind Configuration

Tailwind must:

-   Use **min‑width breakpoints** aligned to the spec:
    -   `xs`: 0–479px
    -   `sm`: 480–767px
    -   `md`: 768–1023px
    -   `lg`: 1024–1439px
    -   `xl`: 1440–1919px
    -   `2xl`: 1920px+
-   Include custom tokens for:
    -   Spacing scale (4/8/12/16/24/32)
    -   Border radius (4/8/12 for small/medium/large)
    -   Elevation (flat/raised/floating) via `boxShadow`
    -   Typography scale (`xs`–`2xl` with consistent line heights)
-   Purge unused styles from all `src/**/*.rs` and template files

Typography stack: `system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif` unless a brand typeface is introduced later.

---

## 3. Information Architecture & Navigation

### 3.1 Top‑Level Navigation

The sidebar must expose the following primary views, in this order:

1. **Dashboard**
2. **Torrents** (primary working view)
3. **Search**
4. **Jobs / Post‑processing**
5. **Settings**
6. **Logs**

Future items:

-   **Indexers** (Phase 2; hidden or disabled for Phase 1)
-   **Library** (Phase 2; hidden or disabled for Phase 1)

### 3.2 Sidebar Layout

-   Left‑hand vertical sidebar, fixed width on desktop
-   Contains:
    -   Revaer logo + wordmark at the top
    -   Navigation menu using **daisyUI `menu menu-lg`** component
    -   Optional footer section for version/build info (later)

Logo requirements:

-   Use the new neon Revaer "R" logo
-   Height approximately 32–36px
-   Positioned with padding (`mt` and `ml`) to visually align with nav items
-   Wordmark "Revaer" to the right of the logo in desktop layout (text‑lg, semibold)

### 3.3 Active and Hover States

-   Active menu item:
    -   Gradient background (purple → magenta)
    -   Left accent bar (approx. 3px wide)
    -   Icon and label in `primary` color
    -   Bold label
-   Hover state:
    -   Text and icon shift to `primary`
    -   No full background fill; subtle highlight only

Icons should be SVGs (Heroicons/Tabler Icons), size 20–24px, with consistent stroke weight.

### 3.4 Page Layout

Each page uses the same structural pattern:

-   **Page header**: title (left), optional actions (right)
-   **Main content area**: scrollable, with standard paddings

No nested scrollbars inside cards. The main content scrolls as a single column.

---

## 4. Responsiveness & Layout Behavior

Revaer uses a **mobile‑first approach** with explicit behavior at each breakpoint.

### 4.1 Breakpoints

-   `xs` / `sm`: mobile
-   `md`: tablet
-   `lg`: laptop
-   `xl`, `2xl`: desktop and ultrawide

All media queries are min‑width.

### 4.2 Behavior by Breakpoint

#### Mobile (xs–sm)

-   Sidebar collapses to **hamburger menu** (daisyUI `drawer` or equivalent pattern)
-   Dashboard stat cards stack vertically
-   Torrent list switches to **card view** (one torrent per card), showing:
    -   Name
    -   Status badge
    -   Progress bar
    -   DL/UL speeds
-   Actions for a torrent (Pause/Resume, Delete, More) rendered as a bottom action row or small icon row inside the card.

#### Tablet (md)

-   Sidebar may remain collapsible but can be shown by default
-   Torrent list can use a compact table with fewer columns visible
-   Dashboard cards arranged in a two‑column grid

#### Laptop (lg)

-   Sidebar fixed and always visible
-   Dashboard metrics use a four‑column grid where space allows
-   Torrent table shows default columns (Name, Status, Progress, DL, UL, Size)

#### Desktop / Ultrawide (xl–2xl)

-   Use available width to:
    -   Expand torrent table columns
    -   Show more metadata inline
    -   Allow split views for detail (later phases)
-   Text content should not exceed a comfortable line length (~110–150 characters)

### 4.3 Table Responsiveness

-   Columns must be priority‑ranked; lower‑priority columns collapse first on small widths
-   Highest priority: Name, Status, Progress, DL/UL
-   Lower priority: Size, Ratio, ETA, Tags, Tracker, Save path
-   Collapsed information may move into:
    -   An expandable row section
    -   A side drawer (later)
    -   A "More details" area in mobile card view

Horizontal scrolling is allowed on small devices but must:

-   Preserve keyboard navigation
-   Show a visual indication that the table scrolls horizontally

### 4.4 Component Responsiveness

-   Dashboard widgets: respond via CSS grid; no fixed pixel widths
-   Torrent detail: on mobile, detail sections are tabs; on desktop, they may be side‑by‑side
-   File tree: accordion list on mobile; traditional tree at `md+`

---

## 5. Dashboard UX Specification

The **Dashboard** is the default landing view. It is composed of four major sections in vertical order:

1. **Top Metrics Row**
2. **Disk Usage & VPN Status**
3. **Events / Tracker Health / Queue Status**
4. **Torrent Table Preview**

### 5.1 Top Metrics Row

The top row displays high‑level metrics in a four‑column grid. Each metric uses a **daisyUI `stat`** component inside a card‑like surface.

Required metrics:

-   Global upload speed
-   Global download speed
-   Active users / sessions
-   Completed torrents / activity summary (configurable)

Each stat shows:

-   Title (e.g., "Global upload")
-   Value (e.g., "31.5 MB/s")
-   Short descriptor (e.g., "Past 60 seconds")

Values should appear stable and not excessively flicker; real‑time updates should be visually smooth (increment changes without jittery animations).

### 5.2 Disk Usage & VPN Status

Two primary cards on the second row:

#### Disk Usage Card

-   Title: "Disk usage"
-   Numeric value and percentage used
-   A single progress bar showing utilization
-   Optional breakdown by path (Phase 2)

#### VPN Status Card

-   Title: "VPN"
-   Status indicator using a **badge** (Connected, Disconnected, Error)
-   Short description (e.g., "All torrent traffic routed through VPN")
-   Optional indicator of current endpoint/location

### 5.3 Events, Tracker Health, Queue

Third row consists of:

1. **Recent Events card**
    - Shows last N events or "No recent events"
    - Each event has a short label and timestamp
2. **Tracker Health card**
    - Aggregated counts: OK, Warning, Error
    - Visualized via colored dots or small inline bars
3. **Queue Status card**
    - Visual representation of queue depth and state
    - Uses simple vertical bars or mini chart; no heavy charting library required in Phase 1

### 5.4 Torrent Table (Preview)

At the bottom of the dashboard, a small torrent table preview is shown (e.g., top 5 torrents by activity). This uses the same table component and styling as the full **Torrents** view, but limited rows.

---

## 6. Torrents View UX Specification

The **Torrents** page is the primary working view.

### 6.1 Columns (Desktop)

Default desktop columns:

-   Name
-   Status
-   Progress
-   ETA
-   Ratio
-   DL speed
-   UL speed
-   Size

Optional/secondary columns (configurable later):

-   Tags / labels
-   Tracker
-   Save path
-   Category / media type

### 6.2 Status Badges

Torrent status must be represented with **daisyUI `badge`** components:

-   Downloading → info style
-   Seeding → success style
-   Paused → warning style
-   Error → error style
-   Completed → neutral or success based on design preference

### 6.3 Progress Representation

Each torrent row includes:

-   Percentage (e.g., `64%`)
-   Thin progress bar (full row width or under the Name or Progress column)

Progress bar must be visually subtle but readable.

### 6.4 Row Interactions

-   Hover: subtle background color change
-   Click: reserved for future detail view (Phase 2); for now, may be no‑op or open a placeholder
-   Bulk selection (Phase 1): optional; if implemented, checkboxes must be aligned in first column

### 6.5 Filtering & Search (Phase 1 UX)

UI must expose:

-   A search input for name/path
-   Basic filters for status and tags

Advanced filters (regex, multi‑field combinations) are allowed but can be behind an "Advanced" toggle.

---

## 7. Adding Torrents UX

An **Add torrent** affordance must exist:

-   Primary button on the Dashboard and Torrents page header
-   The action opens a modal or side panel with:
    -   Field for magnet link / URL
    -   File dropzone for `.torrent` files
    -   Optional fields for category, tags, save path if supported

Validation behaviors:

-   Invalid magnet/URL: inline error under the input
-   Unsupported file type: inline error under dropzone
-   Network/backend error: toast notification + inline message if appropriate

On success:

-   Show a toast (e.g., "Torrent added")
-   Optionally offer a link or button to "View in Torrents" or highlight the new entry

---

## 8. Global Interaction Patterns

### 8.1 Keyboard Shortcuts (Desktop)

Phase 1 must support:

-   `/` → Focus search input (when present)
-   `j` / `k` → Navigate up/down in torrent list (when list has focus)
-   `space` → Pause/Resume focused torrent (if applicable)
-   `delete` → Delete prompt for focused torrent
-   `shift + delete` → Delete + data prompt
-   `p` → Recheck focused torrent

Shortcuts must:

-   Only fire when the relevant context is active (e.g., list focused)
-   Respect forms and text inputs (no hijacking when typing)

### 8.2 Notifications

Use:

-   Toasts/snackbars for transient messages
-   A persistent activity panel or log for longer‑lived system events (Phase 2)

Error messages must:

-   Be human‑readable
-   Include a short machine detail in an expandable section when relevant

---

## 9. Accessibility Requirements

Revaer UI must align with **WCAG 2.1 AA** principles:

-   Sufficient color contrast for all text and interactive elements
-   Keyboard navigability throughout the app
-   Visible focus states for all interactive components
-   Semantic HTML where possible (tables for tabular data, lists for menus)
-   ARIA attributes used appropriately for complex components (menus, dialogs, tabs)

For modals and drawers:

-   Focus must move into the modal when opened
-   Focus must be trapped inside until dismissed
-   Focus must return to the invoking control when closed

---

## 10. Performance & Perceived UX

The UI must:

-   Render the app shell (sidebar + header) quickly, even before data has loaded
-   Use skeletons or loading states instead of blank screens
-   Avoid jarring layout shifts when data arrives

Partial data is allowed:

-   Sidebar and header should always render first
-   Metrics and table contents can show skeleton loaders or placeholders

---

## 11. Documentation & Handover

The engineering team must produce:

-   A living **UI component inventory** documenting:
    -   Sidebar
    -   Page layout
    -   Stat cards
    -   Torrent table
    -   Badges, chips, buttons, inputs, modals
-   Screenshots of:
    -   Dashboard (desktop, lg/xl)
    -   Torrents view (desktop, lg/xl)
-   Notes on any deviations from this spec and rationale

---

## 12. Out of Scope (Phase 1)

-   Indexer configuration UI
-   Library/media‑aware grouping
-   Full torrent detail pages (files/peers/trackers/log as separate view)
-   Mobile‑optimized nav patterns beyond basic responsiveness

These will be defined in subsequent UX Engineering specs.

---

## 13. Summary

This spec defines **what** the Revaer UI must look and feel like for Phase 1 and **how** it must be structured at a UX and component level, without prescribing code. Engineers should be able to:

-   Stand up Yew + Tailwind + daisyUI
-   Implement the app shell, sidebar, dashboard, and torrents view
-   Achieve a near‑pixel‑perfect realization of the intended dark, neon Revaer dashboard

Any ambiguity or deviation should be captured and fed back into the next revision of this spec.
