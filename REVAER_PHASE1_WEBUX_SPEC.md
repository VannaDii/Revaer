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

# 5.3 Brand Palette

### **Brand Palette**

Use the following palette.

## 🎨 **Revaer Full Color Palette (Expanded)**

---

## **Brand Colors**

### **Primary – Deep Nautical Blue (`#265D81`)**

-   `primary-50`: `#E7EFF4`
-   `primary-100`: `#C2D6E4`
-   `primary-200`: `#9CBBD3`
-   `primary-300`: `#76A0C2`
-   `primary-400`: `#4F85B1`
-   `primary-500`: `#265D81` _(base)_
-   `primary-600`: `#1F4D6A`
-   `primary-700`: `#183C52`
-   `primary-800`: `#112B3A`
-   `primary-900`: `#0A1B23`

---

### **Secondary – Muted Violet (`#775A96`)**

-   `secondary-50`: `#F0EBF5`
-   `secondary-100`: `#DAD1E7`
-   `secondary-200`: `#C3B5D8`
-   `secondary-300`: `#A997C7`
-   `secondary-400`: `#8E78B4`
-   `secondary-500`: `#775A96` _(base)_
-   `secondary-600`: `#60497A`
-   `secondary-700`: `#4C3962`
-   `secondary-800`: `#372A48`
-   `secondary-900`: `#241C2F`

---

### **Accent – Bright Blue (`#258BD3`)**

-   `accent-50`: `#E6F2FB`
-   `accent-100`: `#C0DFF8`
-   `accent-200`: `#97C8F2`
-   `accent-300`: `#6DAFEC`
-   `accent-400`: `#4497E4`
-   `accent-500`: `#258BD3` _(base)_
-   `accent-600`: `#1F78B5`
-   `accent-700`: `#196391`
-   `accent-800`: `#134C6C`
-   `accent-900`: `#0D3549`

---

## **Neutral Grays**

### **Light Neutrals**

-   `neutral-50`: `#FFFFFF`
-   `neutral-100`: `#F8F9FA`
-   `neutral-150`: `#F1F3F5`
-   `neutral-200`: `#E9ECEF`
-   `neutral-250`: `#DFE3E6`
-   `neutral-300`: `#DEE2E6`

### **Mid Neutrals**

-   `neutral-400`: `#CED4DA`
-   `neutral-500`: `#ADB5BD`
-   `neutral-600`: `#6C757D`

### **Dark Neutrals**

-   `neutral-700`: `#495057`
-   `neutral-800`: `#343A40`
-   `neutral-900`: `#212529`

---

## **Semantic Colors**

### **Success**

-   `success-100`: `#D9F0EA`
-   `success-500`: `#2F9E7A`
-   `success-700`: `#1E6A51`

### **Warning**

-   `warning-100`: `#FFF4D8`
-   `warning-500`: `#E2AC2F`
-   `warning-700`: `#A4761A`

### **Error**

-   `error-100`: `#FCE6EE`
-   `error-500`: `#C43A61`
-   `error-700`: `#8E2643`

---

## 🌗 **Dark Mode Palette**

### **Base Tokens**

-   `background-dark`: `#121417`
-   `surface-dark`: `#1A1C20`
-   `surface-dark-raised`: `#1F2226`
-   `border-dark`: `#2B2F34`
-   `text-dark-primary`: `#F8F9FA`
-   `text-dark-secondary`: `#C8CDD2`
-   `text-dark-muted`: `#959DA6`

### **Primary (Dark Mode)**

-   `primary-dark-500`: `#4F85B1`
-   `primary-dark-700`: `#2F526F`

### **Secondary (Dark Mode)**

-   `secondary-dark-500`: `#A997C7`
-   `secondary-dark-700`: `#6C5387`

### **Accent (Dark Mode)**

-   `accent-dark-500`: `#4497E4`
-   `accent-dark-700`: `#1E5984`

---

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
