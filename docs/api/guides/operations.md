# Operational Setup & Security

## API key lifecycle

1. Issue keys via settings changesets (`SettingsChangeset::api_keys`).

```json
{
  "api_keys": [
    {
      "op": "upsert",
      "key_id": "automation",
      "label": "ci",
      "secret": "<generated>",
      "rate_limit": { "burst": 60, "per_seconds": 60 },
      "enabled": true
    }
  ]
}
```

2. Secrets are hashed at rest. Rotate keys by supplying a new `secret` value.
3. Disable keys with `{ "op": "upsert", "key_id": "automation", "enabled": false }`.
4. Remove keys with `{ "op": "delete", "key_id": "automation" }`.

All authenticated requests must include `x-revaer-api-key: key_id:secret`.

## Rate limiting

- Token bucket parameters are `burst` (max outstanding requests) and `per_seconds` (refill interval).
- Exceeded limits return `429` Problem+JSON responses plus `x-ratelimit-*` headers.
- Metrics include `api_rate_limit_throttled_total` and `config_guardrail_violations_total`.
- Guardrail violations clear automatically after applying a valid rate limit.

## Secrets and logging

- CLI never prints secrets; API keys must be supplied as `key_id:secret` header values.
- Logs and traces redact secrets; rate-limit warnings include only `key_id`.
- Setup completion requires `x-revaer-setup-token` and returns the generated API key once.

## Docker runtime

- Container runs as a non-root `revaer` user with `/data` and `/config` volumes.
- Healthcheck targets `/health/full`.
- For production, run with a read-only root filesystem and persist `/data` and `/config`.

## SSE and resume

- SSE endpoints accept `Last-Event-ID` for replay. Prefer `/v1/torrents/events` and fall back to `/v1/events/stream`.
- CLI `revaer tail --resume-file <path>` persists event IDs across reconnects.

## Rotation playbook

1. Create a new key with overlapping scope and rate limit.
2. Update automation to use the new secret; confirm via CLI telemetry.
3. Disable the old key and watch `http_requests_total` for drain.
4. Delete the old key once stale traffic drops to zero.
