# API Documentation

This directory hosts HTTP API specifications, the generated OpenAPI document, and usage guides for the Revaer control plane.

## Contents

- `openapi.json` - Generated OpenAPI document (`just api-export`).
- `openapi.md` - How to regenerate and consume the OpenAPI document.
- `guides/` - Scenario-based walkthroughs (bootstrap, operations, telemetry, CLI usage).
- `openapi-gaps.md` - Inventory of router endpoints missing from the OpenAPI spec (should be empty).

## Current Coverage

- **Setup and configuration** - `/admin/setup/*`, `/v1/config`, `/.well-known/revaer.json`.
- **Torrent lifecycle** - `/v1/torrents`, `/v1/torrents/{id}`, `/v1/torrents/{id}/action`, `/v1/torrents/{id}/select`, `/v1/torrents/{id}/options`, plus admin aliases.
- **Authoring and metadata** - `/v1/torrents/create`, `/v1/torrents/{id}/trackers`, `/v1/torrents/{id}/web_seeds`, `/v1/torrents/{id}/peers`.
- **Observability** - `/v1/events`, `/v1/torrents/events`, `/v1/logs/stream`, `/metrics`, `/v1/dashboard`, `/health/full`.
- **Filesystem** - `/v1/fs/browse`.

See `guides/bootstrap.md` for an end-to-end description of the bootstrap lifecycle and runtime orchestration expectations.
