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
    app["App (RevaerApp)"]
    shell["AppShell: nav / theme / locale"]
    dash[DashboardPanel]
    table["TorrentView: virtualized list + filters"]
    add_panel[AddTorrentPanel]
    detail["Detail drawer / tabs"]
    api[API]

    app --> shell
    shell --> dash
    shell --> table
    table --> add_panel
    table --> detail
    add_panel -- "POST /v1/torrents" --> api
    detail -- "PATCH /v1/torrents/{id}" --> api
```

## SSE Event Flow

```mermaid
sequenceDiagram
    participant API as API/SSE
    participant Stream as EventStream
    participant State as UIState
    participant UI as Components

    API->>Stream: SSE events (progress, state, queue, jobs, vpn)
    Stream->>State: Batch apply with backoff + dedupe
    State->>UI: Render updates (virtualized table, dashboard tiles, badges)
    UI->>API: Last-Event-ID on reconnect with filters in SSE query
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
