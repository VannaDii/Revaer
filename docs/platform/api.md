# HTTP API

> REST + SSE surface exposed by `revaer-api`. The OpenAPI document is served at `/docs/openapi.json` and regenerated via `just api-export`.

## Authentication

- **Setup flow** - `/admin/setup/start` is open. `/admin/setup/complete` requires the `x-revaer-setup-token` header with the one-time token returned by setup start. The server refuses setup calls once `app_profile.mode` is `active`.
- **Operator actions** - All `/admin/*` (after setup) and `/v1/*` endpoints require `x-revaer-api-key: key_id:secret`. The middleware validates the key via `ConfigService`, enforces per-key rate limiting, and rejects calls while the instance remains in setup mode.
- **Request correlation** - An optional `x-request-id` header is echoed into tracing spans and surfaced on SSE traffic. The CLI auto-populates this header per invocation.

Error responses follow RFC9457 (`ProblemDetails`) and include `invalid_params` entries when validation pinpoints a JSON pointer within the payload.

## Endpoint inventory (core surface)

### Public (no auth)

- `GET /health`, `GET /health/full`
- `GET /metrics`
- `GET /.well-known/revaer.json`
- `GET /docs/openapi.json`

### Setup and admin

- `POST /admin/setup/start`
- `POST /admin/setup/complete`
- `POST /admin/factory-reset`
- `PATCH /admin/settings` (alias for `PATCH /v1/config`)
- `GET/POST/DELETE /admin/torrents`
- `GET /admin/torrents/{id}`
- `POST /admin/torrents/create`
- `GET /admin/torrents/categories`, `GET /admin/torrents/tags`
- `GET /admin/torrents/{id}/peers`

### Config and auth

- `GET /v1/config` (authenticated snapshot)
- `PATCH /v1/config` (apply `SettingsChangeset`)
- `POST /v1/auth/refresh` (refresh API key)

### Dashboard and filesystem

- `GET /v1/dashboard`
- `GET /v1/fs/browse`

### Torrent lifecycle

- `GET/POST /v1/torrents`
- `GET /v1/torrents/{id}`
- `POST /v1/torrents/{id}/select`
- `PATCH /v1/torrents/{id}/options`
- `POST /v1/torrents/{id}/action`
- `POST /v1/torrents/create`
- `GET /v1/torrents/categories`, `GET /v1/torrents/tags`
- `GET /v1/torrents/{id}/peers`
- `GET/PATCH/DELETE /v1/torrents/{id}/trackers`
- `PATCH /v1/torrents/{id}/web_seeds`

### Events and logs

- `GET /v1/torrents/events` (primary SSE stream)
- `GET /v1/events`, `GET /v1/events/stream` (SSE aliases)
- `GET /v1/logs/stream`

All torrent-managing endpoints ensure the torrent workflow is wired. If the engine is unavailable, the API returns `503 Service Unavailable`.

## Torrent submission (`POST /v1/torrents`)

Required headers: `x-revaer-api-key`. Provide either `magnet` or `metainfo`; the server rejects payloads missing both. Optional fields:

- `download_dir` - Overrides the engine profile's staging directory.
- `sequential` - Enables sequential downloading for this torrent only.
- `tags` / `trackers` - Stored alongside the torrent for filtering and bookkeeping.
- `include` / `exclude` / `skip_fluff` - File selection bootstrap applied before metadata fetch completes.
- `max_download_bps` / `max_upload_bps` - Per-torrent rate limits (bps) passed to the workflow.

On success the server returns `202 Accepted` after dispatching `TorrentWorkflow::add_torrent`. The torrent ID in the payload becomes the canonical identifier.

## Listing and filtering (`GET /v1/torrents`)

Query parameters:

- `limit` (default 50, max 200)
- `cursor` - Base64 token returned in `next`
- `state`, `tracker`, `extension`, `tags`, `name` - Comma-separated filters (case-insensitive)

The response body is `TorrentListResponse` with an optional `next` cursor when additional pages exist.

## Torrent actions (`POST /v1/torrents/{id}/action`)

`type` determines the shape of the body:

```json
{ "type": "remove", "delete_data": true }
{ "type": "sequential", "enable": false }
{ "type": "rate", "download_bps": 1048576, "upload_bps": null }
```

Failures propagate engine errors as `500 Internal Server Error` with a descriptive message in `detail`.

## SSE stream (`GET /v1/torrents/events`)

Headers:

- `x-revaer-api-key`
- Optional `Last-Event-ID` - resuming from a previously stored ID (the CLI stores this via `--resume-file`).

Query parameters:

- `torrent` - Comma-separated UUIDs.
- `event` - Comma-separated event kinds. Valid values include `torrent_added`, `files_discovered`, `progress`, `state_changed`, `completed`, `metadata_updated`, `torrent_removed`, `fsops_started`, `fsops_progress`, `fsops_completed`, `fsops_failed`, `settings_changed`, `health_changed`, `selection_reconciled`.
- `state` - Comma-separated torrent states (`downloading`, `completed`, etc.).

The server maintains a 20-second keep-alive ping and enforces filtering before events hit the wire.

## Health and metrics

- `GET /health` - Primary readiness probe used by orchestration systems. Adds `database` to the degraded list if PostgreSQL is unreachable.
- `GET /health/full` - Returns the deployment revision, build SHA, metrics snapshot (`config_guardrail_violations_total`, `api_rate_limit_throttled_total`, etc.), and torrent queue depth.
- `GET /metrics` - Exposes the same counters for Prometheus scraping.

For the complete schema definitions, consult the generated OpenAPI (`just api-export`).
