# Torrent Flows

Operational views for the torrent lifecycle and the torrent authoring path. These diagrams are reference-only; wire changes must follow the stored-procedure, clamp-before-apply, and observability guardrails in `AGENT.md`.

## Admission → Runtime → FsOps

```mermaid
flowchart TB
    subgraph API["API/CLI"]
        Req["POST /v1/torrents<br/>+ CLI add<br/>- validate payload<br/>- clamp per profile<br/>- hydrate metadata (tags/category/storage)<br/>- normalize selection + limits"]
    end

    subgraph Worker["Worker / Orchestrator"]
        Cmd["EngineCommand::Add<br/>- attach profile snapshot<br/>- derive AddTorrentOptions<br/>- stash selection + metadata for FsOps"]
        Persist["RuntimeStore<br/>- persist metadata/selection<br/>- checkpoint admission state"]
    end

    subgraph Bridge["Bridge / FFI"]
        Opts["EngineOptions/AddTorrentRequest<br/>- listen/download dirs<br/>- per-torrent rate caps<br/>- queue priority / paused<br/>- trackers (profile + request)<br/>- encryption, DHT, LSD flags<br/>- seed mode / add paused"]
        Session["libtorrent session<br/>- apply settings_pack<br/>- add_torrent_params<br/>- start/resume handles"]
    end

    subgraph Engine["Engine Loop"]
        Progress["Native events → EngineEvent<br/>- progress/state<br/>- alert mapping<br/>- tracker status<br/>- errors (listen/storage/peer)"]
        Cache["Per-torrent cache<br/>- rate caps<br/>- trackers<br/>- limits<br/>- tags/category"]
    end

    subgraph FsOps["FsOps Pipeline"]
        Select["Selection reconcile<br/>- honor request selection<br/>- drop unselected paths"]
        Extract["Extract archives (zip/rar/7z/tar.gz)<br/>- optional; skip when not configured<br/>- guardrail missing tools"]
        Flatten["Flatten/move per policy<br/>- copy/move/hardlink<br/>- partfile handling"]
        Perms["chmod/chown/umask<br/>- library root enforcement"]
        Cleanup["Cleanup<br/>- drop patterns<br/>- keep filters<br/>- metadata writeback (.revaer.meta)"]
    end

    Req --> Cmd
    Cmd --> Persist
    Cmd --> Opts
    Opts --> Session
    Session --> Progress
    Progress --> Cache
    Progress -->|Completed event| FsOps
    FsOps -->|Events + metrics| Worker
    Worker -->|Health + SSE| API
```

### Notes

-   Clamping and validation happen before persistence and before libtorrent sees the settings; unknown fields are ignored, unsafe values are clamped.
-   Per-torrent limits (rate caps, queue priority, paused, seed mode) are applied immediately on admission and cached for later verification.
-   FsOps runs on `Completed` with retries; every stage emits events/metrics and degrades health on guardrail breaches (tooling missing, permission errors, latency overruns).

## Torrent Creation (Authoring) Flow

```mermaid
flowchart LR
    Input["Input<br/>- file/dir path<br/>- trackers/web seeds<br/>- piece size (auto/manual)<br/>- private flag<br/>- comment/source<br/>- alignment rules"]
    Stage["Stage & Hash<br/>- walk files with allowlist<br/>- apply size filters<br/>- align pieces<br/>- hash with deterministic order"]
    Meta["Build metainfo<br/>- info dictionary<br/>- tracker tiers<br/>- web seeds<br/>- creation date<br/>- optional dht nodes"]
    Validate["Validate<br/>- size/limit guards<br/>- path length<br/>- private flag vs trackers<br/>- duplicate file detection"]
    PersistMeta["Persist<br/>- .torrent file<br/>- magnet link<br/>- optional signed manifest"]
    Return["Return to caller<br/>- paths + hashes<br/>- effective options<br/>- warnings (skipped files, clamped piece size)"]

    Input --> Stage --> Meta --> Validate --> PersistMeta --> Return
```

### Notes

-   Creation respects the same glob filters and guardrails used by admission to avoid later FsOps surprises (e.g., exclude temporary/system files).
-   When trackers or web seeds are provided, they remain deduplicated and ordered; private torrents skip DHT/PEX automatically.
-   The flow is deterministic: file order, piece sizing, and hashing are reproducible given the same inputs and options.
