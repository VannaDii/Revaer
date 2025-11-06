# Revaer — Phase 1 Engineering Specification

**Revision Date:** 2025-10-26
**Repository:** `VannaDii/Revaer`
**Scope:** Define a complete, executable plan for Phase 1 to deliver a single binary that performs torrent ingestion/control and filesystem post-processing with a DB-backed configuration core, plus a qBittorrent-compatible façade for fast ecosystem interoperability. (Telemetry remains minimal—only what’s required for health and traces.)

> **Why this document exists**
> To replace the Servarr ecosystem with a single-server executable that handles torrenting, library management foundations, and indexer integration in later phases, Revaer must first ship a robust core. This spec gives engineers a concrete blueprint: goals, what’s done, what’s left, sequence of work, acceptance tests, and interfaces—so anyone can implement without ambiguity.

---

## 1) Goals and Non‑Goals (Phase 1)

### 1.1 Goals

-   **Single server binary** that boots in _setup_ mode, then runs steady-state with hot-reloadable config from PostgreSQL.
-   **Torrent engine integration** via engine-agnostic traits and a **libtorrent adapter** (initial engine).
-   **API service** exposing native endpoints for health/config/torrents **and** a **qBittorrent-compatible façade** for basic clients.
-   **Filesystem post‑processing** after download completion: move/rename based on category rules, atomic moves, and staging directories.
-   **Minimal observability**: structured logging and health endpoints; OpenTelemetry wiring is stubbed but off by default.
-   **CI quality gates**: `fmt`, `clippy` (deny warnings), `audit`, `deny`, `udeps`, `llvm-cov`, and workspace build/test.

### 1.2 Non‑Goals (Deferred)

-   Full telemetry/metrics dashboards; advanced analytics and UIs.
-   Full media managers (series/movie/audiobook metadata pipelines).
-   Full indexer management suite (Torznab aggregator, auth rotation).
-   Native web UI beyond a minimal status page (if any) for Phase 1.

---

## 2) Current State (from repo)

This summarizes what is already present in the repository.

-   **Rust workspace & crates scaffolded** (composition root, API, CLI, config, fsops, telemetry, torrent-core, libtorrent adapter).
-   **Tooling present**: `justfile`; quality gates (`deny.toml`); Rust toolchain pin; Dockerfile; mdBook docs skeleton (`book.toml`).
-   **Guiding principles codified**: single source of truth via `DATABASE_URL`, hot reload, safe defaults & setup token gating.
-   **Docs pipeline** ready via mdBook; ADRs location reserved (`docs/adr/`).

> **Cross‑links (repo-relative)**
>
> -   `/Cargo.toml` (workspace)
> -   `/crates/revaer-app` (composition root)
> -   `/crates/revaer-api` (Axum HTTP/SSE services)
> -   `/crates/revaer-cli` (terminal client)
> -   `/crates/revaer-config` (DB-backed settings/migrations)
> -   `/crates/revaer-fsops` (post-processing)
> -   `/crates/revaer-telemetry` (tracing/health hooks)
> -   `/crates/revaer-torrent-core` (traits & DTOs)
> -   `/crates/revaer-torrent-libt` (libtorrent adapter)
> -   `/docs` (mdBook; ADRs, API docs)
> -   `/justfile`, `/deny.toml`, `/book.toml`, `/rust-toolchain.toml`

---

## 3) Gap Analysis

### 3.1 Missing or Incomplete

-   **DB schema & migrations** for core configuration; versioning and initial seed.
-   **Hot‑reload broadcast** from config store to services (LISTEN/NOTIFY or internal bus).
-   **Setup flow**: one-time token, bootstrap endpoints/CLI, safe locked state by default.
-   **Torrent engine core traits** finalized; state model and event stream semantics set.
-   **Libtorrent adapter** linking/build scripts; capability surface mapped to core traits.
-   **qBittorrent-compatible façade**: minimal `/api/v2/*` endpoints implemented.
-   **Filesystem rules engine** and atomic move pipeline (with rollback semantics).
-   **Integration tests** with containerized Postgres and simulated torrent workloads.
-   **ADR set**: configuration model, engine abstraction, façade strategy, fs pipeline.

### 3.2 Technical Debt / Risk

-   Libtorrent Rust bindings may require `cxx`/`bindgen` and careful build steps across platforms; Docker image must include `libtorrent` + `boost` toolchain.
-   Event model must avoid deadlocks: use bounded channels and backpressure strategy.
-   File ops must handle cross-device moves; fallback to copy+rename with checksums.

---

## 4) Architecture Overview

```mermaid
flowchart TD
    subgraph Bin[revaer-app (single binary)]
      A[Config Service (DB)] -- hot reload --> B[API (Axum)]
      A -- hot reload --> C[FS Ops]
      A -- hot reload --> D[Torrent Core]
      D <--> E[Libtorrent Adapter]
      B <-- SSE/Events --> D
      B <--> A
      B <--> C
    end

    subgraph Ext[External]
      PG[(PostgreSQL)]
      Client[Clients & Tools]
      qB[qBittorrent-Compatible Clients]
      FS[(Filesystem)]
    end

    A <--> PG
    C <--> FS
    B <-- /v1/* --> Client
    B <-- /api/v2/* --> qB
```

### 4.1 Processes

-   **Startup (setup mode)** → reads `DATABASE_URL`; checks bootstrap flag; exposes setup endpoints & CLI to initialize admin and base config; then flips to steady-state.
-   **Steady-state** → subscribes to config changes; runs HTTP API; torrent engine loop; fs post-processing worker pool; emits structured events; optional SSE channel.
-   **Shutdown** → graceful: drain channels, flush state, close engine/session.

---

## 5) Data Model (Phase 1)

> **Schema Strategy**: Normalize config and runtime state. Use `sqlx` migrations. Prefer **LISTEN/NOTIFY** for push updates and advisory locks for exclusivity at coordinator boundaries.

### 5.1 Configuration

```sql
-- schema: revaer_config
CREATE SCHEMA IF NOT EXISTS revaer_config;

CREATE TABLE IF NOT EXISTS revaer_config.settings (
  key       TEXT PRIMARY KEY,
  value     JSONB NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  version    BIGINT NOT NULL DEFAULT 1
);

-- Bootstrap/install tracking
CREATE TABLE IF NOT EXISTS revaer_config.installation (
  id          SMALLINT PRIMARY KEY DEFAULT 1,
  is_bootstrapped BOOLEAN NOT NULL DEFAULT FALSE,
  setup_token TEXT, -- one-time token (hashed)
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Keys (examples):**

-   `server.bind`: `{{"host":"0.0.0.0","port":7878}}`
-   `paths.rules`: `{{"categories":{"tv":"...","movies":"...","books":"..."}}}`
-   `torrent.defaults`: `{{"max_conns":200,"limits":{"upload":"...","download":"..."}}}`

### 5.2 Runtime: Torrents & Jobs

```sql
-- schema: revaer_runtime
CREATE SCHEMA IF NOT EXISTS revaer_runtime;

CREATE TYPE revaer_runtime.torrent_state AS ENUM (
  'queued','checking','downloading','seeding','paused','stalled','error','completed'
);

CREATE TABLE IF NOT EXISTS revaer_runtime.torrents (
  info_hash   TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  state       revaer_runtime.torrent_state NOT NULL,
  category    TEXT,
  save_path   TEXT NOT NULL,
  added_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ,
  payload     JSONB NOT NULL DEFAULT '{{}}'::jsonb  -- raw engine snapshot if needed
);

CREATE TYPE revaer_runtime.fs_status AS ENUM ('pending','moving','moved','failed','skipped');

CREATE TABLE IF NOT EXISTS revaer_runtime.fs_jobs (
  id          BIGSERIAL PRIMARY KEY,
  info_hash   TEXT NOT NULL REFERENCES revaer_runtime.torrents(info_hash) ON DELETE CASCADE,
  src_path    TEXT NOT NULL,
  dst_path    TEXT NOT NULL,
  status      revaer_runtime.fs_status NOT NULL DEFAULT 'pending',
  attempt     SMALLINT NOT NULL DEFAULT 0,
  last_error  TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### 5.3 Events (optional, Phase 1 minimal)

-   Use transient in‑memory channels + **SSE** from API; DB persistence not required in Phase 1.

---

## 6) Interfaces

### 6.1 Native HTTP API (v1)

-   `GET /v1/health` → `{{ status: "ok", version, uptime }}`
-   `GET /v1/config` → full resolved config (secure fields redacted)
-   `PATCH /v1/config` → partial update; triggers hot-reload broadcast
-   `POST /v1/setup/apply` → body `{{ setup_token, admin_email?, seed? }}`
-   `GET /v1/torrents` → array of torrents with derived status
-   `POST /v1/torrents/add` → magnet or .torrent upload; optional category/save_path
-   `POST /v1/torrents/{{hash}}/pause|resume|recheck|remove`
-   `GET /v1/events/stream` → **SSE** (torrents, fs-jobs, config-changed)

**Auth**: Phase 1 may rely on loopback-only setup by default; if bound publicly, require setup token; otherwise a minimal static token until full auth arrives in Phase 2.

### 6.2 qBittorrent-Compatible Façade (subset, Phase 1)

Under `/api/v2` implement a minimal but sufficient surface:

-   `POST /api/v2/auth/login` (no-op or static cookie in Phase 1)
-   `GET /api/v2/app/version`, `GET /api/v2/app/webapiVersion`
-   `GET /api/v2/sync/maindata` (project → torrent map, categories, server_state)
-   `GET /api/v2/torrents/info` (basic fields incl. `hash`, `name`, `progress`, `state`)
-   `POST /api/v2/torrents/add` (magnet / file)
-   `POST /api/v2/torrents/pause|resume|delete`
-   `POST /api/v2/transfer/uploadlimit|downloadlimit` (stub accept in Phase 1)

> Façade maps responses from Revaer’s internal model to qBittorrent’s JSON. Ensure idempotent handling and permissive parsing for client compatibility.

### 6.3 Engine Abstractions (Rust)

```rust
// crates/revaer-torrent-core/src/lib.rs
pub enum TorrentState { Queued, Checking, Downloading, Seeding, Paused, Stalled, Error, Completed }

#[derive(Clone, Debug)]
pub struct TorrentSummary {
    pub info_hash: String,
    pub name: String,
    pub state: TorrentState,
    pub progress: f32,           // 0.0..=1.0
    pub save_path: String,
    pub category: Option<String>,
    pub added_at: std::time::SystemTime,
    pub completed_at: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug)]
pub enum EngineEvent {
    Added(TorrentSummary),
    Updated(TorrentSummary),
    StateChanged { info_hash: String, state: TorrentState },
    Completed { info_hash: String },
    Removed { info_hash: String },
    Error { info_hash: Option<String>, message: String },
}

#[async_trait::async_trait]
pub trait TorrentEngine: Send + Sync {
    async fn add_magnet(&self, uri: &str, category: Option<&str>, save_path: Option<&str>) -> anyhow::Result<String>; // returns info_hash
    async fn pause(&self, info_hash: &str) -> anyhow::Result<()>;
    async fn resume(&self, info_hash: &str) -> anyhow::Result<()>;
    async fn remove(&self, info_hash: &str, with_data: bool) -> anyhow::Result<()>;
    async fn list(&self) -> anyhow::Result<Vec<TorrentSummary>>;
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EngineEvent>;
}
```

### 6.4 Filesystem Pipeline (Rust)

```rust
pub struct FsRule { pub category: String, pub template: String /* e.g., "{name}" */ }

pub trait FsOps {
    fn plan(&self, summary: &TorrentSummary, rules: &[FsRule]) -> Option<(std::path::PathBuf, std::path::PathBuf)>;
    async fn execute(&self, info_hash: &str, src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()>;
}
```

---

## 7) Implementation Plan (by crate)

### 7.1 `revaer-config`

-   **Migrations**: create `settings` and `installation` tables; ship `sqlx::migrate!()`.
-   **API**: `get(key) -> serde_json::Value`, `set(key, value)`, `subscribe()` for watchers.
-   **Hot reload**: `LISTEN/NOTIFY` channel `revaer_config_changed`; broadcast version bumps.
-   **CLI**: `revaer --config set server.bind='{{...}}'` for fast ops.

**Done when**:

-   Migrations run on boot; config cache populates on start and updates on NOTIFY.
-   Unit tests cover read/write; integration tests cover bootstrap to steady-state.

### 7.2 `revaer-torrent-core`

-   Finalize state model and DTOs; define `TorrentEngine` trait; map progress/state enums.
-   Event channel via `tokio::broadcast` with bounded capacity and lag handling policy.

**Done when**:

-   Mock engine passes contract tests (add/pause/resume/remove/list + event flow).

### 7.3 `revaer-torrent-libt`

-   Use `cxx` or `libtorrent-sys` bindings to `libtorrent-rasterbar`.
-   Implement trait: `add_magnet`, pause/resume/remove/list; translate engine events.
-   Provide build scripts and Docker stage to fetch/build libtorrent and link correctly.

**Done when**:

-   Adapter passes core contract tests on Linux (Docker) and macOS (local).

### 7.4 `revaer-fsops`

-   Implement planner (category → destination template) and executor (atomic move; cross-device fallback copy+rename; checksum guard optional).
-   Retry queue with exponential backoff; updates `fs_jobs`.

**Done when**:

-   End-to-end test: complete torrent triggers plan+execute and final path materialization.

### 7.5 `revaer-api` (Axum)

-   Native `v1` endpoints; SSE event stream that multiplexes engine+fsops+config.
-   Compatibility router `/api/v2/*` (qBittorrent subset) behind feature flag `compat-qb`.
-   JSON error model; CORS config off by default; bind address from config.

**Done when**:

-   Contract tests: JSON shapes stable; E2E tests with mock engine and real Postgres.

### 7.6 `revaer-app`

-   Composition root wiring the config cache, engine adapter, fsops, and API.
-   Startup mode check; setup token + unlock flow; graceful shutdown.

**Done when**:

-   `cargo run` with only `DATABASE_URL` starts in setup; after unlock, steady-state.

### 7.7 `revaer-cli`

-   `setup unlock --token <one-time>`
-   `config get/set`
-   `torrents add/pause/resume/remove/list` (delegates to API where possible)

**Done when**:

-   Smoke tests against a local server instance pass on CI.

### 7.8 `revaer-telemetry` (minimal in Phase 1)

-   Provide `tracing` subscriber and health check helpers.
-   OpenTelemetry export behind a feature flag; default disabled.

**Done when**:

-   Logs structured and consistent; `/v1/health` reports green under normal ops.

---

## 8) Priority & Execution Order (optimize for least rework)

1. **Config + Migrations (`revaer-config`)**
2. **Torrent Core trait & state model (`revaer-torrent-core`)**
3. **Libtorrent Adapter (`revaer-torrent-libt`)**
4. **API v1 (read-only) + health + list (`revaer-api`)**
5. **API v1 mutations (add/pause/resume/remove)**
6. **Filesystem Ops (planner + executor)**
7. **Event bus + SSE** (wire torrents + fsops + config change)
8. **qBittorrent façade (subset)** for `/torrents/*`, `/sync/maindata`
9. **Setup flow + one-time token** (lockdown by default)
10. **CLI**
11. **ADR authoring** for the decisions implemented above

> **Parallelism**: 2–3 engineers can parallelize (config+API skeleton, core+adapter, fsops) once the trait contracts are frozen.

---

## 9) Acceptance Tests (Definition of Done)

### 9.1 System

-   With only `DATABASE_URL` set, server starts in setup mode; `/v1/health` returns `{ setup: true }` and restricted routes reject mutation.
-   Applying setup token flips to steady-state; `/v1/health` returns `{ setup: false }`.
-   Adding a magnet via `/v1/torrents/add` shows in `/v1/torrents`; transitions to `completed` in a simulated environment; fsops moves payload atomically.
-   `/api/v2/torrents/info` returns qB-compatible JSON for at least one active and one completed torrent.

### 9.2 Performance

-   Listing torrents (<1k items) returns within 150ms P50 on Docker Linux host; SSE event latency < 200ms P95 under steady-state.

### 9.3 Reliability

-   Engine crash/restart → reattach to session without data loss; fsops resumes pending jobs; idempotent moves.

### 9.4 Security

-   Default bind is loopback or unauthenticated endpoints are minimized; setup token is one-time and hashed in DB; feature-flagged façade disabled by default for public binds.

---

## 10) Build, Packaging, and CI

### 10.1 Docker

-   Multi-stage: Rust builder → slim runtime image with `libtorrent` + `libstdc++` + minimal `openssl`/`ca-certificates` as needed.
-   Env: only `DATABASE_URL` required. Optional `RUST_LOG`.

### 10.2 Justfile (targets to verify)

-   `fmt`, `lint`, `test`, `build`, `udeps`, `audit`, `deny`, `cov`, `ci`, `docs`, `docs-serve`

### 10.3 CI

-   Matrix Linux/macOS (Linux required); cache Cargo; run DB in service container; run integration tests and E2E with mock engine; build image artifact.

---

## 11) Risk Register & Mitigations

-   **FFI fragility**: lock known-good `libtorrent` version; publish a `build.rs` that checks headers and fails fast with actionable help.
-   **Filesystem edge cases**: detect cross-device moves; if fallback copy+rename fails, surface precise errors and mark job `failed` with retry policy.
-   **Façade drift**: pin supported qB Web API version; maintain tight mapping tests with golden JSON fixtures.

---

## 12) Appendix A — qBittorrent façade mapping (Phase 1 subset)

| qBittorrent API           | Revaer native                       | Notes                          |
| ------------------------- | ----------------------------------- | ------------------------------ |
| `/api/v2/torrents/add`    | `POST /v1/torrents/add`             | Accept magnet/file; map fields |
| `/api/v2/torrents/pause`  | `POST /v1/torrents/{{hash}}/pause`  | Multi-hash support via loop    |
| `/api/v2/torrents/resume` | `POST /v1/torrents/{{hash}}/resume` |                                |
| `/api/v2/torrents/delete` | `POST /v1/torrents/{{hash}}/remove` | honour `deleteFiles`           |
| `/api/v2/torrents/info`   | `GET /v1/torrents`                  | shape-mapping layer            |
| `/api/v2/sync/maindata`   | aggregate from engine/config        | cache small server_state       |

---

## 13) Appendix B — ADR Stubs to write

1. **Config as SSOT** (DB-backed, LISTEN/NOTIFY)
2. **Engine Abstraction + Events**
3. **Libtorrent Adapter Approach (cxx vs bindgen)**
4. **qBittorrent-Compatible Façade scope & version pin**
5. **Filesystem post‑processing transaction semantics**

---

## 14) Appendix C — Test Matrix

-   Linux + Docker (libtorrent) — required
-   macOS local — nice to have (build-only)
-   Postgres versions 14–16

---

## 15) Out‑of‑Scope for Phase 1 (but planned)

-   Indexer management (Torznab aggregator) → **Phase 2**
-   Media managers (Series/Movie/Audiobook) metadata & decision engines → **Phase 2/3**
-   Rich UI & dashboards → **Phase 3**

---

### End of Phase 1 Engineering Specification
