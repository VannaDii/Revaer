# Revaer UI — Dashboard UX Engineering Specification (Phase 1)

**Version:** 1.0
**Owner:** UX Engineering
**Audience:** UI Engineers, Yew Developers, QA, Product
**Stack:** Yew + TailwindCSS + daisyUI
**Scope:** Dashboard-only detailed implementation specification
**Derived From:** Phase 1 Master UX Specification

This document defines the **complete and authoritative** engineering requirements for the Revaer Dashboard. All UI engineers must follow these specifications exactly when implementing the Dashboard view.

---

# 1. Visual Intent & Experience Principles

The Revaer Dashboard must:

-   Present system and torrent activity **at a glance**, with a clean, modern, neon-noir aesthetic.
-   Match the attached reference mock **pixel for pixel** in layout hierarchy, spacing, tone, and component interaction.
-   Use **no custom CSS files**—all styling achieved via Tailwind utility classes and daisyUI components.
-   Reflect the Revaer Dark Theme palette defined in the main Phase 1 spec.
-   Prioritize legibility, consistent vertical rhythm, and predictable layout.
-   Convey sophistication and confidence without visual noise.

This screen establishes the **visual identity** for all future screens.

---

# 2. Structural Overview

The Dashboard layout consists of:

1. **App Shell** (Sidebar + Header Row)
2. **Dashboard Content Area**, comprised of:
    - Top Metrics Row
    - Disk Usage & VPN Row
    - Events / Tracker Health / Queue Status Row
    - Torrent Table Preview

All content must exist inside:

-   A fixed-width sidebar on the left.
-   A right-hand scrollable content column with standard padding.
-   Standard top-level padding: `px-6 pt-6 pb-12`.

---

# 3. App Shell Specification

## 3.1 Sidebar

The Sidebar is defined in the master spec. For the Dashboard specifically:

-   The **Dashboard** menu item must appear active.
-   Use **daisyUI `menu menu-lg`**.
-   Apply active styling:
    -   Background: `nav-active-bg` (dark violet-blue)
    -   Overlay: low-opacity left-edge gradient (brand magenta → violet)
    -   3px accent strip: absolutely positioned left, full height
    -   Icon: gradient-filled (`nav-icon-active`)
    -   Text: `primary` color
-   Spacing: Sidebar inner padding must visually align with header title.

## 3.2 Header Row

Placed at the top of the content area (NOT part of sidebar):

-   `flex justify-between items-center w-full`
-   **Left:** Page title → “Dashboard”
    -   Typography: `text-2xl font-semibold text-primary`
-   **Right:** Connection Status Pill
    -   Component: **daisyUI `badge`** or **`btn btn-sm btn-outline`**
    -   Icon: check-circle (Tabler or Heroicons)
    -   Color: `accent` (cyan-on-dark)
    -   Label: “Connected” (Phase 1 static text)

Spacing:

-   Title row sits `mt-6 mb-4` before the metrics grid.

---

# 4. Layout Grid Specification (Desktop)

Use a consistent 4-column Tailwind grid system:

```
grid grid-cols-4 gap-6
```

Column spans:

-   **Metrics Row:** 1 span per metric (total 3 or 4)
-   **Disk Usage:** `col-span-3`
-   **VPN Card:** `col-span-1`
-   **Recent Events:** `col-span-2`
-   **Tracker Health:** `col-span-1`
-   **Queue Status:** `col-span-1`
-   **Table Preview:** `col-span-4`

Vertical spacing between rows: `mt-6`.

---

# 5. Top Metrics Row Specification

## 5.1 Component

Each metric uses:

-   **daisyUI `card`** with `bg-surface-1`.
-   Inside: **daisyUI `stat`**.

## 5.2 Content Structure

Inside each `stat`:

-   `stat-title` — label (e.g. “Global upload”)

    -   Color: `text-secondary`
    -   Font size: small

-   `stat-value` — main metric (e.g. “31.5 MB/s”)

    -   Color: `text-primary`
    -   Font size: large, bold

-   `stat-desc` — sublabel (e.g. “MB/s”)

    -   Color: `text-muted`

-   **Metric Line** (required):
    -   A thin horizontal bar underneath the stat block
    -   Track: `progress-bg`
    -   Fill: `progress-primary` or gradient `progress-primary → progress-secondary`
    -   Height: `h-[3px]`
    -   Rounding: `rounded-full`
    -   Placement: `mt-4`

## 5.3 Required Metrics

The Dashboard must display **four** metric cards:

1. **Global upload speed**
2. **Global download speed**
3. **Torrent counts (Active / Paused / Completed)** — primary value shows Active; Paused and Completed appear as inline subvalues.
4. **Active users/sessions** — count of active devices/users (matches master spec’s fourth metric).

---

# 6. Disk Usage & VPN Status Row Specification

## 6.1 Disk Usage Card — `col-span-3`

Component: **daisyUI `card`** with `bg-surface-1`.

Content elements:

-   Title: “Disk usage” (`text-secondary`)
-   Primary Value: e.g. “2.1 TB” (`text-primary text-xl font-semibold`)
-   Usage Bar:
    -   Track: `progress-bg`
    -   Fill: gradient `progress-primary → progress-secondary`
    -   Height: `h-[6px]`
    -   Rounding: medium
-   Optional descriptor: e.g. “68% used” (`text-muted text-sm`)

Card must visually match the wide-left card in the reference dashboard.

## 6.2 VPN Status Card — `col-span-1`

Component: **daisyUI `card`** with compact proportions.

Content layout (inside `card-body`):

-   Small VPN icon block:

    -   Could be a `btn btn-square btn-sm` or a rounded div using `bg-surface-2`.
    -   Icon: lock/shield icon (Tabler/Heroicons)

-   Title block:

    -   Title: “VPN” (`text-secondary text-sm`)
    -   Status text: “Connected” / “Error” / “Disconnected”
        -   Colors: `success`, `error`, `warning` respectively

-   Right chevron icon: indicates click-through (future)
    -   Alignment: `ml-auto`
    -   Color: muted slate

Spacing inside card must mirror reference: vertically centered content, minimal padding.

---

# 7. Events / Tracker Health / Queue Status Row Specification

## 7.1 Recent Events Card — `col-span-2`

Component: **daisyUI `card`** with `bg-surface-2`.

Content:

-   Title: “Recent Events”
-   If no events: centered text “No recent events” using `text-muted`.
-   If events exist (future): simple list of rows with timestamp + description.

Typography must remain minimal to match the image.

## 7.2 Tracker Health Card — `col-span-1`

Component: **daisyUI `card`** with `bg-surface-2`.

Content:

-   Title: “Tracker Health”
-   Legend of three statuses:
    -   Ok — green dot (`success`)
    -   Warning — yellow dot (`warning`)
    -   Error — red dot (`error`)

Dot implementation: small 8–10px circles using semantic tokens.

## 7.3 Queue Status Card — `col-span-1`

Component: **daisyUI `card`** with `bg-surface-2`.

Content:

-   Title: “Queue Status”
-   A bar visualization:
    -   Container: `flex items-end gap-2 h-24`
    -   Bars: 4–6 vertical bars
        -   Width: `w-3`
        -   Height: varied between `h-6` and `h-16`
        -   Color: `queue-bar` for primary bars; `queue-bar-muted` for variation

This row must match the visual from the reference mock exactly.

---

# 8. Torrent Table Preview Specification

## 8.1 Wrapper

-   Component: **daisyUI `card`** with `bg-surface-1` and `card-body p-0`.
-   Full-width (`col-span-4`).

## 8.2 Table Component

-   Use **daisyUI `table table-zebra w-full`**.
-   Header columns: Name, Status, Progress, ETA, Ratio, DL, UL, Size.

## 8.3 Table Row Requirements

Each row includes:

-   **Name** — left-aligned text-primary; truncate long names.
-   **Status** — daisyUI `badge` with semantic token:

    -   Downloading → `badge-info`
    -   Seeding → `badge-success`
    -   Paused → `badge-warning`
    -   Error → `badge-error`
    -   Completed → neutral or `badge-success`

-   **Progress**:

    -   Thin bar using `progress-bg` (track) and `progress-primary` (fill).
    -   Optional percent text in muted color.

-   **ETA** — text-secondary
-   **Ratio** — text-secondary, 2 decimal clamp.
-   **DL / UL** — numbers with units in `text-secondary`.
-   **Size** — text-secondary, binary units.

Row hover: subtle background shift to `bg-table-row-alt`.

Rows are non-clickable in Phase 1.

---

# 9. Dashboard Interactions & Behavior

-   Hover: cards and table rows show slight color shift only.
-   Focus: all interactive elements must show a `focus-ring` outline.
-   No modals or drawers appear from the Dashboard in Phase 1.
-   Connection pill has no click behavior yet.
-   Torrent table preview is static for Phase 1; API-driven content may be added later.
-   Skeleton states:
    -   Metrics: grey bars matching the structure of metric cards
    -   Disk/VPN cards: grey blocks where values would be
    -   Table: grey rows or muted “Loading…”

---

# 10. Implementation Notes

-   Dashboard spacing, colors, and component usage must strictly follow the Revaer design tokens.
-   No arbitrary hex values may appear in the markup; use theme tokens.
-   Consistency is mandatory — this page sets the baseline for all future Revaer pages.
