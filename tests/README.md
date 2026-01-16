# Playwright E2E tests

## Overview

- `just ui-e2e` provisions temp databases, runs API suites for both auth modes, then runs UI tests.
- API tests execute before UI tests to surface backend failures first.
- Configuration is loaded from `tests/.env` (overrides are allowed via env vars).

## Requirements

- Ports `7070` (API) and `8080` (UI) must be free (the runner will stop existing Revaer dev servers, but other services must be stopped manually).
- Postgres must be reachable via `E2E_DB_ADMIN_URL`.
  - If the host is local, `just ui-e2e` will call `just db-start` to bootstrap Docker.

## Run

```bash
just ui-e2e
```

## Configuration

- `E2E_BASE_URL` / `E2E_API_BASE_URL`: UI/API base URLs (defaults to `http://localhost:8080` and `http://localhost:7070`).
- `E2E_DB_ADMIN_URL`: admin connection string used to create temp DBs.
- `E2E_DB_PREFIX`: prefix for temp DB names.
- `E2E_FS_ROOT`: filesystem root used by `/v1/fs/browse` and torrent authoring tests (relative paths resolve against the repo root; default is `.server_root/library`).
- `E2E_SESSION_PATH`: session file used by global setup/teardown.
- `E2E_BROWSERS`: UI browser list (`chromium`, `firefox`, `webkit`).
