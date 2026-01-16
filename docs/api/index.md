# API Documentation

This directory hosts HTTP API specifications, generated OpenAPI documents, and usage guides for the Revaer control plane.

## Contents
- `schema/` – Published OpenAPI payloads and supporting artefacts.
- `guides/` – Scenario-based walkthroughs (bootstrap, hot reload validation, torrent lifecycle).
- `examples/` – HTTP request/response samples captured from real workflows.
- `openapi-gaps.md` – Routes present in the codebase but missing from the OpenAPI spec.

## Current Coverage
- **Setup & configuration** – `/admin/setup/*` and `/admin/settings` flows with CLI parity.
- **Orchestration** – `/admin/torrents` (POST/DELETE/GET) for submitting or removing torrents, plus `/admin/torrents/{id}` for status inspection.
- **Observability** – `/v1/events` SSE stream (tested for replay/keep-alive) and `/metrics` Prometheus surface with torrent gauges.

See `guides/bootstrap.md` for an end-to-end description of the bootstrap lifecycle, background workers, and error handling expectations.

## Next Steps
- Capture worked examples for torrent status reconciliation (list + selective GET).
- Provide troubleshooting recipes for common workflow failures (engine unavailable, filesystem policy rejection).
- Expand SSE consumer documentation with incremental backfill strategies.
