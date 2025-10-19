# 007 – API Key Security & Rate Limiting

- Status: Accepted
- Date: 2025-10-17

## Context
- API keys were previously verified but not throttled, allowing abusive clients to starve the control plane and masking guard-rail violations.
- Operators need guard-rail metrics, health events, and documentation describing key lifecycle, rate limits, and rotation workflows.
- CLI tooling must respect the same security posture, including masking secrets and surfacing authentication failures with actionable errors.

## Decision
- Each API key stores a JSON rate limit (`burst`, `per_seconds`) validated by `ConfigService`; token-bucket state is maintained per key inside the API layer.
- Requests exceeding the configured budget return `429 Too Many Requests` Problem+JSON responses, increment Prometheus counters (`api_rate_limit_throttled_total`), and emit `HealthChanged` events when guard rails (e.g., unlimited keys) are breached.
- CLI authentication mandates `key_id:secret`, redacts secrets in logs, and propagates `x-request-id` so operators can correlate requests with server-side traces.
- CI enforces MSRV and Docker security gates to ensure build artefacts respect the security baseline.

## Consequences
- Compromised or runaway keys are contained, preventing control-plane denial-of-service and providing clear telemetry for incident response.
- Documentation now includes API key rotation steps, rate-limit expectations, and remediation guidance for guard-rail events.
- The API and CLI remain aligned by sharing auth context types and telemetry primitives.

## Verification
- Unit tests cover rate-limit parsing and token-bucket behaviour; integration tests assert `429` responses and CLI exit codes.
- `/health/full` exposes rate-limit metrics, and the Docker image runs as a non-root user with health checks hitting the authenticated endpoints.
