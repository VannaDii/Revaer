# Web UI Flows & Diagrams

Visual references for the Phase 1 UX: navigation, component wiring, SSE handling, and torrent lifecycle. Use these diagrams when extending the UI or adding tests.

## Navigation Flow

```mermaid
flowchart LR
    Nav[Sidebar / Hamburger] --> Dash[Dashboard]
    Nav --> Torrents[Torrents]
    Nav --> Search[Search]
    Nav --> Jobs[Jobs / Post-processing]
    Nav --> Settings[Settings]
    Nav --> Logs[Logs]
    Torrents --> Detail[Detail Drawer / Tabs]
    Detail --> Files[Files]
    Detail --> Peers[Peers]
    Detail --> Trackers[Trackers]
    Detail --> Events[Event Log]
    Detail --> Info[Metadata]
```

## Component Graph

```mermaid
flowchart TB
    App[App (RevaerApp)]
    Shell[AppShell (nav, mode/theme toggle, locale picker)]
    Dash[DashboardPanel]
    Table[TorrentView (virtualized list + filters)]
    Add[AddTorrentPanel]
    Detail[Detail Drawer/Tabs]

    App --> Shell
    Shell --> Dash
    Shell --> Table
    Table --> Add
    Table --> Detail
    Add -->|POST /v1/torrents| API
    Detail -->|PATCH /v1/torrents/{id}| API
```

## SSE Event Flow

```mermaid
sequenceDiagram
    participant API as API / SSE
    participant Stream as EventStream
    participant State as UI State (torrents, queue, jobs, vpn)
    participant UI as Components

    API-->>Stream: SSE events (torrent_progress, torrent_state, queue_status, jobs_update, vpn_state)
    Stream->>State: Batch apply with backoff + dedupe
    State-->>UI: Render updates (virtualized table, dashboard tiles, badges)
    UI-->>API: Last-Event-ID on reconnect; filters included in SSE query
```

## Torrent Lifecycle (UI Perspective)

```mermaid
stateDiagram-v2
    [*] --> Added : magnet/upload
    Added --> Queueing : server-side validation
    Queueing --> Downloading
    Downloading --> Checking : recheck or hash
    Downloading --> Completed : 100% + seeding ready
    Checking --> Downloading : if data matches
    Completed --> FsOps : move/rename per policy
    FsOps --> Seeding
    Seeding --> Completed : ratio met / stop rules
    Completed --> Removed : delete (+data optional)
```

## Interaction Notes

- SSE disconnect overlay shows last event timestamp, retry countdown (1s→30s exponential with jitter), and diagnostics (network mode, reason).
- Table virtualization is mandatory beyond 500 rows; virtual scroll must preserve keyboard focus order and pinned columns.
- Mobile detail view uses tabs (Files, Peers, Trackers, Log, Info); desktop uses split panes so file tree + metadata stay visible together at xl+.
