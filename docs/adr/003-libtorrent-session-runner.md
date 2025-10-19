# 003 – Libtorrent Session Runner Architecture

- Status: Accepted
- Date: 2025-10-16

## Context
- The current `revaer-torrent-libt` crate is a stub that simulates torrent actions without touching libtorrent, preventing real downloads, fast-resume, or alert handling.
- Phase One requires a production-grade engine: a single async task must own the libtorrent session, persist fast-resume data/selection state, debounce high-volume alerts, and surface health to the event bus.
- The engine must enforce rate limits and selections within libtorrent, react within two seconds of configuration changes, and survive restarts by restoring torrents from `resume_dir`.

## Decision
- Introduce a dedicated `SessionWorker` spawned by `LibtorrentEngine::new`. It owns the libtorrent `Session`, receives `EngineCommand` messages, and emits `EngineEvent`s via an internal channel that feeds the shared `EventBus`.
- Wrap the libtorrent FFI in a thin adapter trait (`LibtSession`) to encapsulate blocking calls (`add_torrent`, `pause`, `set_sequential`, `apply_rate_limits`, `file_priorities`, alert polling). The real implementation uses `tokio::task::spawn_blocking` to call into C++ safely.
- Add a `FastResumeStore` service that reads/writes `.fastresume` blobs plus JSON metadata (selection, priorities, download directory, sequential flag) inside `resume_dir`. On startup the worker loads the store, attempts to match existing handles, and emits reconciliation events if the stored state diverges.
- Run an `AlertPump` loop that waits on libtorrent `alerts_waitnotify`, drains all alerts, and funnels them through an `AlertTranslator` that converts them into domain `EngineEvent`s (`FilesDiscovered`, `Progress`, `StateChanged`, `Completed`, `Error`). A `ProgressCoalescer` throttles updates to 10 Hz per torrent.
- Integrate health tracking: fatal session errors transition the engine into a degraded state and emit both `HealthChanged` and per-torrent `Error` events. The worker attempts limited restarts with exponential back-off before marking the engine unhealthy.
- Rate limit updates from `EngineCommand::UpdateLimits` and configuration watcher updates call into libtorrent immediately; a watchdog verifies application within two seconds and logs warnings if the session reports stale caps.

## Consequences
- The engine crate gains clear separation between command handling, libtorrent FFI, alert translation, and persistence, making it easier to test components in isolation using mock `LibtSession` implementations.
- Persisted state in `resume_dir` enables crash-restart flows to resume downloads, leveraging libtorrent fastresume and our own selection metadata.
- Debouncing progress events reduces SSE pressure while preserving responsiveness; coalescing happens before events hit the shared bus.
- Health reporting integrates with the existing telemetry crate, providing operators visibility into session failures or missing dependencies (e.g., absent resume directory).

## Follow-up
- Maintain regression coverage for the `libtorrent` feature path, ensuring fast-resume reconciliation and guard-rail health events remain stable.
- Track upstream libtorrent upgrades and refresh the operator documentation whenever the resume layout or dependency expectations shift.
