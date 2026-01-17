# Torrent Flows

Operational views for the torrent lifecycle and the torrent authoring path. These diagrams are reference-only; wire changes must follow the stored-procedure, clamp-before-apply, and observability guardrails in `AGENT.md`.

## Admission -> Runtime -> FsOps

```mermaid
flowchart TB
    subgraph API["API/CLI"]
        Req["POST /v1/torrents\nPATCH /v1/torrents/{id}/options\nPOST /v1/torrents/{id}/select\nPATCH /v1/torrents/{id}/trackers\nPATCH /v1/torrents/{id}/web_seeds\n- validate payload\n- clamp per profile\n- hydrate metadata (tags/category/storage)\n- normalize selection + limits"]
    end

    subgraph Worker["Worker / Orchestrator"]
        Cmd["EngineCommand::Add\n- attach profile snapshot\n- derive AddTorrentOptions\n- stash selection + metadata for FsOps"]
        Persist["RuntimeStore\n- persist metadata/selection\n- checkpoint admission state"]
    end

    subgraph Bridge["Bridge / FFI"]
        Opts["EngineOptions/AddTorrentRequest\n- listen/download dirs\n- per-torrent rate caps\n- queue priority / paused\n- trackers (profile + request)\n- encryption, DHT, LSD flags\n- seed mode / add paused"]
        Session["libtorrent session\n- apply settings_pack\n- add_torrent_params\n- start/resume handles"]
    end

    subgraph Engine["Engine Loop"]
        Progress["Native events -> EngineEvent\n- progress/state\n- alert mapping\n- tracker status\n- errors (listen/storage/peer)"]
        Cache["Per-torrent cache\n- rate caps\n- trackers\n- limits\n- tags/category"]
    end

    subgraph FsOps["FsOps Pipeline"]
        Select["Selection reconcile\n- honor request selection\n- drop unselected paths"]
        Extract["Extract archives (zip/rar/7z/tar.gz)\n- optional; skip when not configured\n- guardrail missing tools"]
        Flatten["Flatten/move per policy\n- copy/move/hardlink\n- partfile handling"]
        Perms["chmod/chown/umask\n- library root enforcement"]
        Cleanup["Cleanup\n- drop patterns\n- keep filters\n- metadata writeback (.revaer.meta)"]
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

- Clamping and validation happen before persistence and before libtorrent sees the settings; unknown fields are ignored, unsafe values are clamped.
- Per-torrent limits (rate caps, queue priority, paused, seed mode) are applied immediately on admission and cached for later verification.
- FsOps runs on `Completed` with retries; every stage emits events/metrics and degrades health on guardrail breaches (tooling missing, permission errors, latency overruns).

## Torrent creation (authoring) flow

```mermaid
flowchart LR
    Input["Input\n- file/dir path\n- trackers/web seeds\n- piece size (auto/manual)\n- private flag\n- comment/source\n- alignment rules"]
    Stage["Stage & Hash\n- walk files with allowlist\n- apply size filters\n- align pieces\n- hash with deterministic order"]
    Meta["Build metainfo\n- info dictionary\n- tracker tiers\n- web seeds\n- creation date\n- optional dht nodes"]
    Validate["Validate\n- size/limit guards\n- path length\n- private flag vs trackers\n- duplicate file detection"]
    PersistMeta["Persist\n- .torrent file\n- magnet link\n- optional signed manifest"]
    Return["Return to caller\n- paths + hashes\n- effective options\n- warnings (skipped files, clamped piece size)"]

    Input --> Stage --> Meta --> Validate --> PersistMeta --> Return
```

### Notes

- Creation respects the same glob filters and guardrails used by admission to avoid later FsOps surprises (exclude temporary/system files).
- When trackers or web seeds are provided, they remain deduplicated and ordered; private torrents skip DHT/PEX automatically.
- The flow is deterministic: file order, piece sizing, and hashing are reproducible given the same inputs and options.
- API endpoint: `POST /v1/torrents/create` (admin alias: `POST /admin/torrents/create`).
