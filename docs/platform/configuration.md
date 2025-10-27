# Configuration Surface

> Canonical reference for the PostgreSQL-backed settings documents that drive Revaer’s runtime behaviour.

Revaer persists all operator-facing configuration inside the `settings_*` tables. The API (`ConfigService`) exposes strongly-typed snapshots that are consumed by the API server, torrent engine, filesystem pipeline, and CLI. Every change flows through a `SettingsChangeset`, ensuring a single validation path whether commands originate from the setup flow or the admin API.

## Snapshot Components

The `/ .well-known/revaer.json` endpoint and `revaer setup complete` CLI command both return the same structure:

```json
{
    "revision": 42,
    "app_profile": {
        /* see below */
    },
    "engine_profile": {
        /*…*/
    },
    "fs_policy": {
        /*…*/
    },
    "api_keys": [
        {
            "key_id": "admin",
            "label": "bootstrap",
            "enabled": true,
            "rate_limit": null
        }
    ]
}
```

### App Profile (`settings_app_profile`)

| Field            | Type                    | Description                                                                                                |
| ---------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------- |
| `id`             | UUID                    | Singleton identifier for the current document.                                                             |
| `instance_name`  | string                  | Human readable label surfaced in the CLI after setup.                                                      |
| `mode`           | `"setup"` or `"active"` | Gatekeeper for the authentication middleware. Setup requests are rejected once the system enters `active`. |
| `version`        | integer                 | Optimistic locking counter maintained by `ConfigService`.                                                  |
| `http_port`      | integer                 | Published TCP port for the API server.                                                                     |
| `bind_addr`      | string (IPv4/IPv6)      | Listen address for the API server.                                                                         |
| `telemetry`      | object                  | Free-form map for logging + metrics toggles (e.g. `log_level`, `prometheus`).                              |
| `features`       | object                  | Feature switches such as `fs_extract`, `par2`, `sse_backpressure`.                                         |
| `immutable_keys` | array                   | List of fields that cannot be mutated via patches (`ConfigError::ImmutableField`).                         |

### Engine Profile (`settings_engine_profile`)

| Field                                 | Type     | Description                                                                 |
| ------------------------------------- | -------- | --------------------------------------------------------------------------- |
| `implementation`                      | string   | Currently `libtorrent`. Used to select the torrent workflow implementation. |
| `listen_port`                         | integer? | Optional external listen port override for the engine.                      |
| `dht`                                 | bool     | Enables/disables the DHT module.                                            |
| `encryption`                          | string   | Encryption requirement (`require`, `prefer`, etc.).                         |
| `max_active`                          | integer? | Cap on concurrently-active torrents; `null` means unlimited.                |
| `max_download_bps` / `max_upload_bps` | integer? | Global rate limits applied by the engine.                                   |
| `sequential_default`                  | bool     | Default sequential downloading behaviour for new torrents.                  |
| `resume_dir`                          | string   | Filesystem location where fast-resume artefacts are stored.                 |
| `download_root`                       | string   | Directory used for in-progress torrent payloads.                            |
| `tracker`                             | object   | Tracker configuration (user-agent, announce overrides).                     |

### Filesystem Policy (`settings_fs_policy`)

| Field                           | Type    | Description                                                                                                                                          |
| ------------------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `library_root`                  | string  | Destination directory for completed artefacts.                                                                                                       |
| `extract`                       | bool    | Whether completed payloads are extracted.                                                                                                            |
| `par2`                          | string  | `off`, `verify`, or `repair` depending on PAR2 behaviour.                                                                                            |
| `flatten`                       | bool    | Collapses single-file directories when moving into the library.                                                                                      |
| `move_mode`                     | string  | `copy`, `move`, or `hardlink` semantics for the FsOps pipeline.                                                                                      |
| `cleanup_keep` / `cleanup_drop` | array   | Glob patterns retaining or removing files during cleanup.                                                                                            |
| `chmod_file` / `chmod_dir`      | string? | Optional octal permissions applied to outputs; when omitted the pipeline derives modes from `umask` (defaults to `0o666/0o777`).                     |
| `owner` / `group`               | string? | Optional ownership override resolved against system users/groups on Unix platforms; unsupported on non-Unix systems (FsOps emits a guarded failure). |
| `umask`                         | string? | Umask applied during FsOps and used to derive default file/directory modes when explicit chmod directives are absent.                                |
| `allow_paths`                   | array   | Allowed staging/library paths the pipeline accepts.                                                                                                  |

### API Keys & Secrets

Patches can create, update, or revoke keys and named secrets. The request format mirrors `SettingsChangeset`:

```jsonc
{
    "api_keys": [
        {
            "op": "upsert",
            "key_id": "admin",
            "label": "primary",
            "enabled": true,
            "secret": "optional-override",
            "rate_limit": { "burst": 10, "per_seconds": 1 }
        }
    ],
    "secrets": [
        { "op": "set", "name": "libtorrent.passphrase", "value": "..." }
    ]
}
```

The API server enforces bucketed rate limits if `rate_limit` is supplied (`burst` per `per_seconds`). Invalid field names or mutations against `immutable_keys` yield RFC9457 `ProblemDetails` responses with an `invalid_params` array matching the JSON pointer returned by `ConfigError`.

## Change Workflows

-   **Setup** – `POST /admin/setup/start` issues a one-time token. `POST /admin/setup/complete` consumes that token, applies the provided `SettingsChangeset`, forces `app_profile.mode` to `active`, and returns the hydrated snapshot along with the generated API key (also echoed in the CLI output).
-   **Ongoing updates** – `PATCH /admin/settings` (CLI: `revaer settings patch --file changes.json`) requires an API key and supports partial documents. Any field omitted from the payload remains untouched.
-   **Snapshot access** – `GET /.well-known/revaer.json` (no auth) and `GET /health/full` both return the revision and enable automation to verify configuration drift. Automation and dashboards can poll these endpoints without authenticating.

Revaer publishes `SettingsChanged` events on every successful mutation, ensuring subscribers refresh in-memory caches without polling.
