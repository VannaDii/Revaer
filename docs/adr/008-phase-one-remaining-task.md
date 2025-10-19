# 008 – Phase One Remaining Delivery (Task Record)

- Status: In Progress
- Date: 2025-10-17

## Motivation
- Implement the outstanding Phase One scope: per-key rate limiting, CLI parity (telemetry, exit codes), packaging, documentation, and CI gates required by `docs/phase-one-remaining-spec.md` and `AGENT.md`.

## Design Notes
- Introduced `ConfigService::authenticate_api_key` returning rate-limit metadata, validated JSON payloads, and persisted canonical token-bucket configuration.
- Added `ApiState::enforce_rate_limit` with per-key token buckets, guard-rail health publication, Prometheus counters, and Problem+JSON `429` responses.
- CLI now builds `reqwest` clients with default `x-request-id`, standardises exit codes (`0/2/3`), and emits optional telemetry events when `REVAER_TELEMETRY_ENDPOINT` is set.
- Created a multi-stage Dockerfile (non-root runtime, healthcheck, docs bundling) with `just` recipes for building and scanning.
- Expanded CI with release artefact, Docker, and MSRV jobs that call the new `just` targets.

## Test Coverage Summary
- Added unit tests for rate-limit parsing and token-bucket behaviour (`revaer-config`, `revaer-api`).
- Existing integration suites exercise Problem+JSON responses, SSE replay, and CLI HTTP interactions.
- Runbook (`docs/runbook.md`) supports manual verification of FsOps, rate limits, and guard rails.

## Observability Updates
- Prometheus now exposes `api_rate_limit_throttled_total`; `/health/full` includes the counter and degrades when guard rails fire.
- CLI telemetry emits JSON events (command, outcome, trace id, exit code) to configurable endpoints.
- Documentation adds telemetry reference, operations guide, and release checklist for operators.

## Risk & Rollback
- Rate-limit enforcement is isolated to `require_api_key`; rollback by removing `enforce_rate_limit` call if unexpected throttles occur.
- Docker image/builder changes are gated via `just docker-build` and `just docker-scan`; revert by restoring previous absence of Docker packaging.
- CI additions run after core jobs and can be disabled via workflow changes if they fail unexpectedly.

## Dependency Rationale
- No new Rust crates were introduced. Docker scanning uses `trivy` via CI and manual recipe; it is optional for local development.
