# Phase One Remaining Engineering Specification

## Objectives

- Deliver a production-ready public interface (HTTP API, SSE, CLI) for torrent orchestration.
- Ship FsOps-backed artefacts through API, CLI, telemetry, and documentation with demonstrable reliability.
- Produce release artefacts (containers, binaries, documentation) that satisfy existing security, observability, and quality gates.

## Scope Overview

1. **Public HTTP API & SSE Enhancements**
   - `/v1/torrents` CRUD-style endpoints with cursor pagination, filtering, torrent actions, file selection updates, rate adjustments, and Problem+JSON responses.
   - SSE stream upgrades: Last-Event-ID replay, subscription filters, duplicate suppression, jitter-tolerant reconnect logic.
   - `/health/full` exposing engine/FsOps/config readiness, dependency metrics, and revision metadata.
   - Regenerated OpenAPI (JSON + examples) reflecting the full public surface.

2. **CLI Parity**
   - Commands covering list/status/select/action/tail flows with shared filtering + pagination options.
   - SSE-backed `tail` command with Last-Event-ID resume, dedupe, and retry semantics aligned with the API.
   - Problem+JSON error output, structured exit codes (`0` success, `2` validation, `>2` runtime failures).

3. **Packaging & Documentation**
   - Release-ready Docker image (non-root, readonly FS, volumes, healthcheck) bundling API server + docs.
   - Provenance-signed binaries for supported architectures, plus GitHub Actions workflows for build, docker, msrv, and coverage gates.
   - Updated ADRs, runbook, user guides, OpenAPI artefacts, and release checklist referencing the telemetry and security posture.
   - Documentation of new metrics/traces/guardrails (config watcher latency, FsOps events, API counters).

## Security & Observability Requirements (Cross-Cutting)

- All new API routes enforce API-key authentication with per-key rate limiting and guard-rail metrics.
- Problem+JSON responses are mandatory; eliminate `unwrap`/panic paths and include `invalid_params` pointers on validation failure.
- Trace propagation from API → engine → FsOps; CLI should emit/propagate TraceId when available.
- Metrics: extend existing Prometheus registry with route labels, FsOps step counters, config watcher latency/failure gauges, and rate-limiter guardrails.
- Health degradation events (`Event::HealthChanged`) must accompany any new guard-rail/latency breach or pipeline failure.
- CLI commands should mask secrets in logs and optionally emit telemetry when configured (`REVAER_TELEMETRY_ENDPOINT`).

## Detailed Work Breakdown

### 1. Public API & SSE

**Design Considerations**
- Introduce DTO module (`api::models`) for request/response structs to share with the CLI.
- Cursor pagination: encode UUID/timestamp as opaque cursor in `next` token; align Last-Event-ID semantics with event stream IDs.
- Filtering: support state, tracker, extension, tags, and name substring; guard invalid combinations with Problem+JSON.
- SSE filtering: permit query parameters for torrent subset, replays based on event type/state.

**Implementation Tasks**
- Routes:
  - `POST /v1/torrents` – magnet or .torrent upload (streamed, payload size guard).
  - `GET /v1/torrents` – cursor pagination + filters.
  - `GET /v1/torrents/{id}` – detail view with FsOps metadata.
  - `POST /v1/torrents/{id}/select` – file selection update with validation.
  - `POST /v1/torrents/{id}/action` – pause/resume/remove (with data), reannounce, recheck, sequential toggle, rate limits.
- SSE:
  - Accept `Last-Event-ID` header, deduplicate by event ID, filter streams by torrent ID/state.
  - Simulate jitter/disconnects in tests (`tokio::time::pause`, `transport::Stream`).
- Health endpoint:
  - Aggregate config watcher metrics (latency, failures), FsOps status, engine guardrails, revision hash.
- Problem+JSON mapping for all new errors with `invalid_params` pointer data.
- OpenAPI:
  - Regenerate spec covering new endpoints, Problem responses, SSE details, and sample payloads.
- Testing:
  - Unit tests for filter parsing, DTO validation, Problem+JSON outputs.
  - Integration tests using `tower::Service` harness for each route.
  - SSE reconnection tests with simulated delays and Last-Event-ID resume.
  - `/health/full` integration test verifying new fields and degraded scenarios.

### 2. CLI Parity

**Design Considerations**
- Reuse DTOs from API models; consider shared crate/module for request structs and Problem+JSON parsing.
- Introduce output formatting with optional JSON/pretty table modes.
- Provide configuration via env vars and CLI flags; align defaults with API (e.g., `REVAER_API_URL`, `REVAER_API_KEY`).

**Implementation Tasks**
- Commands:
  - `revaer ls` – list torrents, support pagination (`--cursor`, `--limit`), filters (state/tracker/extension/tags).
  - `revaer status <id>` – torrent detail view, optional follow mode.
  - `revaer select <id>` – send selection rules from file/JSON (validate before submit).
  - `revaer action <id>` – actions (`pause`, `resume`, `remove`, `remove-data`, `reannounce`, `recheck`, `sequential`, `rate`).
  - `revaer tail` – SSE tail with Last-Event-ID persist (local file) and dedupe.
- Problem+JSON handling:
  - Standardised pretty printer summarising `title`, `detail`, `invalid_params`; respect exit codes.
- Telemetry:
  - Optional metrics emission (success/failure counters) when telemetry endpoint configured.
- Testing:
  - Integration tests using `httpmock` to assert HTTP interactions and exit codes.
  - SSE tail tests with mocked stream delivering duplicates/disconnects.
  - Snapshot tests for JSON outputs (ensuring deterministic fields).

### 3. Packaging & Documentation

**Design Considerations**
- Multi-stage Docker build: compile with Rust image, run on minimal base (distroless/alpine/ubi) with non-root user.
- Healthcheck script hitting `/health/full` with timeout.
- Release workflows should run on GitHub Actions with provenance metadata (supply-chain compliance).

**Implementation Tasks**
- Dockerfile + `Makefile`/`just` target:
  - Build release binary, copy `docs/api/openapi.json`, set `/app` as workdir.
  - Define volumes for data/config, create user `revaer`, configure entrypoint.
- GitHub Actions (update `.github/workflows`):
  - `build-release`: run `just build-release`, `just api-export`, attach binaries/docs.
  - `docker`: build image, run `docker scan` (`trivy`/`grype`), and push on release tags.
  - `msrv`: run `just fmt lint test` with pinned toolchain (documented in workflow).
  - `cov`: ensure `just cov` gate passes (≥80% lines/functions).
- Documentation:
  - ADRs: update `003-libtorrent-session-runner`, add FsOps design ADR, API/CLI contract ADR, security posture update (API keys, rate limits).
  - Runbook: scripted scenario covering bootstrap → torrent add → FsOps pipeline → restart resume → rate throttle adjustments → degraded health simulation → recovery.
  - User guides: CLI usage, metrics/telemetry reference, operational setup (keys, rate limits, config watcher health).
  - OpenAPI: regenerate JSON, include sample Problem+JSON payloads and SSE description.
  - Release checklist: steps to run `just ci`, verify coverage, run docker scan, execute runbook, and tag release.
- Testing:
  - Validate Docker container runtime (healthcheck, volume mounts, non-root permissions).
  - Perform coverage review ensuring new tests bring line/function coverage ≥80%.
  - Execute runbook; capture logs/metrics and link in docs.

## Cross-Cutting Deliverables

- API key lifecycle (issue/rotate/revoke) extended with per-key rate limiting, recorded in telemetry and docs.
- Config watcher telemetry integrated into `/health/full` and metrics registry.
- CLI and API emit guard-rail telemetry on violations (loopback enforcement, FsOps errors, rate-limit breaches).
- All new code paths covered by unit/integration tests; follow-up to update `just cov` gating.
- Documentation kept up-to-date with implementation details and tested flows.

## Sequencing (Suggested)

1. Build API models and endpoints (foundation for CLI).
2. Implement SSE enhancements while adding API integration tests.
3. Extend CLI commands leveraging shared DTOs.
4. Embed telemetry (metrics/traces) throughout API/CLI/FsOps changes.
5. Stand up Docker build + CI workflows.
6. Update ADRs, runbook, user guides, OpenAPI, and release checklist.
7. Execute full QA cycle (coverage, docker scan, runbook, manual verification) and prepare for release tagging.

## Acceptance Criteria

- `just lint`, `just test`, `just cov` and full `just ci` pass locally and in CI.
- Coverage (lines + functions) ≥ 80% across workspace.
- Docker image passes security scan with zero unwaived high severity findings.
- Runbook executed end-to-end; results referenced in documentation.
- OpenAPI specification and CLI docs match implemented behaviour.
- Release checklist completed with artefacts attached (binaries, Docker image, OpenAPI, docs).
