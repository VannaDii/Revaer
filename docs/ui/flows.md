# Web UI Flows and Diagrams

Visual references for the Phase 1 UX: navigation, component wiring, SSE handling, and torrent lifecycle. Use these diagrams when extending the UI or adding tests.

## Navigation flow

```mermaid
flowchart LR
    Nav["Sidebar / Drawer"] --> Dash[Dashboard]
    Nav --> Torrents[Torrents]
    Nav --> Logs[Logs]
    Nav --> Health[Health]
    Nav --> Settings[Settings]
    Torrents --> Detail["Detail route /torrents/:id"]
    Detail --> Overview[Overview]
    Detail --> Files[Files]
    Detail --> Options[Options]
```

## Component graph

```mermaid
flowchart TB
    app["App (RevaerApp)"]
    shell["AppShell: nav / theme / locale"]
    dash[Dashboard]
    torrents["Torrents list + detail"]
    settings[Settings]
    logs[Logs]
    health[Health]
    api[API]

    app --> shell
    shell --> dash
    shell --> torrents
    shell --> settings
    shell --> logs
    shell --> health

    dash -- "GET /v1/dashboard" --> api
    torrents -- "GET /v1/torrents" --> api
    torrents -- "GET /v1/torrents/{id}" --> api
    torrents -- "POST /v1/torrents/{id}/action" --> api
    torrents -- "PATCH /v1/torrents/{id}/options" --> api
    torrents -- "POST /v1/torrents/{id}/select" --> api
    torrents -- "SSE /v1/torrents/events" --> api
    logs -- "SSE /v1/logs/stream" --> api
    health -- "GET /health/full" --> api
```

## SSE event flow

```mermaid
sequenceDiagram
    participant UI as UI
    participant Fetch as Fetch Stream
    participant API as API/SSE
    participant State as Store

    UI->>Fetch: build URL + headers (x-revaer-api-key, Last-Event-ID)
    Fetch->>API: GET /v1/torrents/events (fallback /v1/events/stream)
    API-->>Fetch: SSE frames
    Fetch->>State: parse + batch updates
    State->>UI: render list, detail, dashboard, health badges
    UI->>Fetch: reconnect with backoff and resume id
```

## Torrent lifecycle (UI perspective)

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

## Interaction notes

- SSE disconnect overlay shows last event timestamp, retry countdown (1s to 30s exponential with jitter), and diagnostics (auth mode, reason).
- Table virtualization is required beyond 500 rows; virtual scroll must preserve keyboard focus order and pinned columns.
- Mobile detail view uses tabs (Overview, Files, Options); desktop uses a split layout so overview and options stay visible together at lg+.
