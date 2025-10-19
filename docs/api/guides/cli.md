# CLI Usage Guide

The `revaer` CLI provides parity with the public API while honouring the same authentication, pagination, and SSE semantics.

## Global Flags

- `--api-url` (`REVAER_API_URL`): Base URL for the API (`http://127.0.0.1:7070` by default).
- `--api-key` (`REVAER_API_KEY`): Supplied as `key_id:secret`; required for all non-setup commands.
- `--timeout` (`REVAER_HTTP_TIMEOUT_SECS`): Request timeout in seconds (default `10`).
- `REVAER_TELEMETRY_ENDPOINT`: Optional endpoint for emitting JSON telemetry events (command, outcome, trace id, exit code).

Each command propagates a unique `x-request-id` header, enabling correlation with API logs and traces.

## Commands

| Command | Description |
| --- | --- |
| `setup start` / `setup complete` | Bootstraps a new deployment, issuing one-time tokens and initial config. |
| `settings patch --file settings.json` | Applies JSON changesets via the configuration service. |
| `torrent add <magnet|.torrent>` | Adds a torrent with optional name/ID; accepts JSON file selection hints. |
| `torrent remove <id>` | Removes torrents (with `--delete-data` to purge files). |
| `ls` | Lists torrents with cursor pagination (`--limit`, `--cursor`) and filters (`--state`, `--tracker`, `--extension`, `--tags`). |
| `status <id>` | Retrieves detailed torrent info (files, rates, metadata). |
| `select <id>` | Updates include/exclude rules and file priorities from CLI arguments or JSON payloads. |
| `action <id>` | Runs torrent actions (`pause`, `resume`, `remove`, `reannounce`, `recheck`, `sequential`, `rate`). |
| `tail` | Connects to the SSE stream with filters (`--torrent`, `--event`, `--state`), resuming from `--resume-file`. |

All commands support `--output json` for machine-readable output (default `table`).

## Exit Codes

- `0`: Success.
- `2`: Validation/Problem+JSON errors (e.g., bad filter, authentication missing).
- `3`: Runtime failures (network issues, 5xx/429 responses).

The CLI prints RFC9457 details for non-success responses, including any `invalid_params` pointers returned by the API.

## Telemetry

When `REVAER_TELEMETRY_ENDPOINT` is set, each command posts a JSON payload containing:

```json
{
  "command": "ls",
  "outcome": "success",
  "trace_id": "...",
  "exit_code": 0,
  "timestamp_ms": 1739731200000
}
```

Failures include an additional `message` field mirroring the CLI error output. These events allow operators to trace automation flows without parsing stdout.
