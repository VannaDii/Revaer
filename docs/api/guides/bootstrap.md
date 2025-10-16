# Bootstrap & Workflow Lifecycle

This guide explains how the Revaer application stitches configuration, orchestration, and observability components together once the binary starts.

## Startup flow
- **Configuration service** – `ConfigService::watch_settings` returns the initial snapshot and a watcher. The bootstrap task applies the snapshot to the API listener and spawns a background task that reacts to subsequent engine/FS policy changes.
- **Orchestrator wiring** – `spawn_libtorrent_orchestrator` creates the torrent engine, filesystem post-processing service, and an orchestration task that subscribes to the shared event bus. Each event updates an in-memory torrent catalogue (exposed through the `TorrentInspector` trait) so API consumers can query live status.
- **API exposure** – `ApiServer::new` receives a `TorrentHandles` bundle (workflow + inspector). The REST surface uses the workflow handles for `/admin/torrents` writes and the inspector for `/admin/torrents` reads, while telemetry gauges (`active_torrents`, `queue_depth`) are refreshed on each call.

## Runtime updates
- **Config watcher** – On every settings change the watcher:
  1. Applies the latest filesystem policy (`update_fs_policy`).
  2. Pushes the engine profile into the orchestration layer (`update_engine_profile`), which propagates DHT, port, and throttling changes through the `EngineConfigurator` trait.
- **Event propagation** – The SSE endpoint (`/v1/events`) streams the shared event bus. Tests cover replay semantics and idle keep-alive behaviour to ensure UI consumers remain in sync.

## Error handling
- **Workflow guard rails** – If the API receives admin torrent requests while the workflow is unavailable, the handlers return `503 Service Unavailable` (tested via `dispatch_torrent_add/remove`). CLI commands surface the same problem as validation errors.
- **Setup token/auth failures** – Setup routes retain existing problem responses (`409` for already configured systems, `401` for invalid tokens).
- **Filesystem post-processing** – Errors raised during post-processing emit `fsops_failed` events; the orchestration task logs the failure and continues processing subsequent events.

## Operational signals
- **Logging** – Torrent mutations (`POST /admin/torrents`, `DELETE /admin/torrents/{id}`) emit structured `info!` logs with torrent identifiers.
- **Metrics** – The API refreshes `active_torrents` and `queue_depth` gauges whenever inspectors are queried. Keep the `/metrics` endpoint scraped to track orchestration pressure.
- **SSE replay** – Ensure clients persist the last delivered `event.id`; the API honours the `Last-Event-ID` header to deliver missed lifecycle events after reconnects.
