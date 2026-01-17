# Configuration Surface

> Canonical reference for the PostgreSQL-backed settings documents that drive Revaer's runtime behavior.

Revaer persists operator-facing configuration inside the `settings_*` tables. The API (`ConfigService`) exposes strongly typed snapshots consumed by the API server, torrent engine, filesystem pipeline, and CLI. Every change flows through a `SettingsChangeset`, ensuring a single validation path whether commands originate from the setup flow or the admin API.

## Snapshot components

The `/.well-known/revaer.json` endpoint, the authenticated `GET /v1/config` route, and the `revaer config get` CLI command all return the same structure:

```json
{
  "revision": 42,
  "app_profile": {
    "...": "..."
  },
  "engine_profile": {
    "...": "..."
  },
  "engine_profile_effective": {
    "...": "..."
  },
  "fs_policy": {
    "...": "..."
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

`engine_profile_effective` is the normalized engine profile (clamped limits, derived defaults, warnings applied) used by the orchestrator.

## App profile (`settings_app_profile`)

| Field | Type | Description |
| --- | --- | --- |
| `id` | UUID | Singleton identifier for the current document. |
| `instance_name` | string | Human-readable label surfaced in the UI and CLI. |
| `mode` | `setup` or `active` | Gatekeeper for authentication middleware and setup flow. |
| `auth_mode` | `api_key` or `none` | API access policy; `none` allows anonymous access on local networks only. |
| `version` | integer | Optimistic locking counter maintained by `ConfigService`. |
| `http_port` | integer | Published TCP port for the API server. |
| `bind_addr` | string (IPv4/IPv6) | Listen address for the API server. |
| `telemetry` | object | Structured telemetry config (`level`, `format`, `otel_enabled`, `otel_service_name`, `otel_endpoint`). |
| `label_policies` | array | Per-category/tag policy overrides (download dir, rate limits, queue position). |
| `immutable_keys` | array | Fields that cannot be mutated via patches (`ConfigError::ImmutableField`). |

## Engine profile (`settings_engine_profile`)

### Network and transport

- `implementation` - engine identifier (`libtorrent` or `stub`).
- `listen_port` and `listen_interfaces` - incoming listener configuration.
- `ipv6_mode` - `disabled`, `prefer`, or `require`.
- `enable_lsd`, `enable_upnp`, `enable_natpmp`, `enable_pex` - discovery toggles (default off).
- `dht`, `dht_bootstrap_nodes`, `dht_router_nodes` - DHT configuration.
- `outgoing_port_min` / `outgoing_port_max` - optional port range for outgoing connections.
- `peer_dscp` - optional DSCP/TOS codepoint (0-63) for peer sockets.

### Privacy and protocol controls

- `anonymous_mode`, `force_proxy`, `prefer_rc4`.
- `allow_multiple_connections_per_ip`.
- `enable_outgoing_utp`, `enable_incoming_utp`.

### Limits and scheduling

- `max_active`, `max_download_bps`, `max_upload_bps`.
- `seed_ratio_limit`, `seed_time_limit`.
- `connections_limit`, `connections_limit_per_torrent`.
- `unchoke_slots`, `half_open_limit`, `optimistic_unchoke_slots`.
- `stats_interval_ms`, `max_queued_disk_bytes`.
- `alt_speed` (caps and optional schedule).

### Behavior

- `sequential_default`.
- `auto_managed`, `auto_manage_prefer_seeds`, `dont_count_slow_torrents`.
- `super_seeding`, `strict_super_seeding`.
- `choking_algorithm`, `seed_choking_algorithm`.

### Storage

- `resume_dir`, `download_root`.
- `storage_mode`, `use_partfile`.
- `disk_read_mode`, `disk_write_mode`, `verify_piece_hashes`.
- `cache_size`, `cache_expiry`, `coalesce_reads`, `coalesce_writes`, `use_disk_cache_pool`.

### Tracker and filtering

- `tracker` (user-agent, announce overrides).
- `ip_filter` (inline rules plus optional remote blocklist).
- `peer_classes` (per-class caps and throttles).

## Filesystem policy (`settings_fs_policy`)

| Field | Type | Description |
| --- | --- | --- |
| `library_root` | string | Destination directory for completed artifacts. |
| `extract` | bool | Whether completed payloads are extracted. |
| `par2` | string | `disabled`, `verify`, or `repair`. |
| `flatten` | bool | Collapse single-file directories when moving into the library. |
| `move_mode` | string | `copy`, `move`, or `hardlink`. |
| `cleanup_keep` / `cleanup_drop` | array | Glob patterns retaining or removing files. |
| `chmod_file` / `chmod_dir` | string? | Optional octal permissions applied to outputs. |
| `owner` / `group` | string? | Optional ownership override (Unix only). |
| `umask` | string? | Umask used to derive default permissions. |
| `allow_paths` | array | Allowed staging/library paths. |

## API keys and secrets

Patches can create, update, or revoke keys and named secrets. The request format mirrors `SettingsChangeset`:

```json
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

## Telemetry toggle

Revaer boots with structured logging and Prometheus metrics by default. OpenTelemetry export remains opt-in: set `REVAER_ENABLE_OTEL=true` alongside your `revaer-app` process (optionally overriding `REVAER_OTEL_SERVICE_NAME` and `REVAER_OTEL_EXPORTER`) to attach the stubbed tracing layer. When the flag is absent, no OpenTelemetry dependencies are activated.

## Change workflows

- **Setup** - `POST /admin/setup/start` issues a one-time token. `POST /admin/setup/complete` consumes that token, applies the provided `SettingsChangeset`, forces `app_profile.mode` to `active`, and returns the hydrated snapshot along with the generated API key.
- **Ongoing updates** - `PATCH /v1/config` (CLI: `revaer config set --file changes.json`) requires an API key and supports partial documents. Any field omitted from the payload remains untouched. The legacy `/admin/settings` alias remains for compatibility.
- **Snapshot access** - `GET /.well-known/revaer.json` (no auth), `GET /v1/config` (API key), `GET /health/full`, and `revaer config get` return the current revision so automation and dashboards can verify configuration drift without shell access.

Revaer publishes `SettingsChanged` events on every successful mutation, ensuring subscribers refresh in-memory caches without polling.
