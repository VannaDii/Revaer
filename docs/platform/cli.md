# CLI Reference

> `revaer-cli` provides parity with the API for setup, configuration management, torrent lifecycle, and observability.

## Global Flags & Environment

| Flag | Environment | Default | Description |
| --- | --- | --- | --- |
| `--api-url <URL>` | `REVAER_API_URL` | `http://127.0.0.1:7070` | Base URL for API requests. |
| `--api-key <key_id:secret>` | `REVAER_API_KEY` | _none_ | Required for all post-setup commands that mutate or read torrents. |
| `--timeout <secs>` | `REVAER_HTTP_TIMEOUT_SECS` | `10` | Per-request HTTP timeout. |
| `--output <table\|json>` | _none_ | `table` | Output format for structured responses (`json` is script-friendly). |

Each invocation bubbles a unique `x-request-id` through the API; the CLI also emits optional telemetry events when `REVAER_TELEMETRY_ENDPOINT` is set.

## Setup Flow

### `revaer setup start [--issued-by <label>] [--ttl-seconds <secs>]`

- Calls `POST /admin/setup/start`.
- Prints the plaintext token followed by its ISO8601 expiry.
- Use `--issued-by` to tag the token source (defaults to `api`).

### `revaer setup complete --instance <name> --bind <addr> --port <port> --resume-dir <path> --download-root <path> --library-root <path> --api-key-label <label> [--api-key-id <id>] [--passphrase <value>] [--token <token>]`

- Loads the setup token either from `--token` or `REVAER_SETUP_TOKEN`.
- Builds a `SettingsChangeset` containing the app profile, engine profile, filesystem policy, API key, and optional secret.
- Forces `app_profile.mode = "active"`.
- Echoes the generated API key (`key_id:secret`) on success; store it securely before continuing.

## Configuration Maintenance

### `revaer settings patch --file <path>`

- Reads a JSON file containing a partial `SettingsChangeset`.
- Requires an API key.
- Returns a formatted `ProblemDetails` message if validation fails (immutable fields, unknown keys, etc.).

## Torrent Lifecycle

### `revaer torrent add <magnet|.torrent> [--name <label>] [--id <uuid>]`

- Accepts a magnet URI or a filesystem path to a `.torrent`.
- Automatically base64-encodes torrent files for the API.
- Optional overrides: `--name` sets the human-friendly label; `--id` lets you supply a deterministic UUID instead of the auto-generated value.

### `revaer torrent remove <uuid>`

- Issues `POST /v1/torrents/{id}/action` with `{ "type": "remove" }`.
- Use the more general `action` command for `delete_data` semantics.

### `revaer ls [--limit <n>] [--cursor <token>] [--state <state>] [--tracker <url>] [--extension <ext>] [--tags <tag1,tag2>] [--name <fragment>]`

- Lists torrents with the same filters supported by the REST API.
- Default output is a table summarising id, name, state, and progress.
- Add `--output json` to emit the raw `TorrentListResponse`.

### `revaer status <uuid>`

- Returns a detailed view of a single torrent.
- Add `--output json` to view the full `TorrentDetail` (including file metadata when available).

### `revaer select <uuid> [--include <glob,glob>] [--exclude <glob,glob>] [--skip-fluff] [--priority index=priority,…]`

- Updates file-selection rules via `POST /v1/torrents/{id}/select`.
- `--priority` accepts repeated `index=priority` pairs (`skip|low|normal|high`) mapped onto the engine’s `FilePriority`.

### `revaer action <uuid> <pause|resume|remove|reannounce|recheck|sequential|rate> [--delete-data] [--enable <bool>] [--download <bps>] [--upload <bps>]`

- One-stop entry point for all torrent actions.
- `sequential` toggles sequential downloads via `--enable true|false`.
- `rate` updates per-torrent bandwidth caps (bps). Provide `--download` and/or `--upload`.
- `remove` honours `--delete-data`.

## Event Streaming

### `revaer tail [--torrent <id,id>] [--event <kind,kind>] [--state <state,state>] [--resume-file <path>] [--retry-secs <n>]`

- Connects to `/v1/events` using SSE.
- Filters match the API query parameters and enforce UUID/event-kind validation before the request is made.
- When `--resume-file` is supplied, the CLI persists the last event ID across reconnects so the stream can resume after transient failures.
- `--retry-secs` controls the backoff between reconnect attempts (default: 5 seconds).

All torrent commands require an API key. The CLI surfaces API problems exactly as the server returns them, including RFC9457 validation errors and rate-limit responses (`429 Too Many Requests` with retry metadata in the body).
