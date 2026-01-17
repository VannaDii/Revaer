# Revaer

Revaer is a data-driven media orchestration platform that centralizes configuration, telemetry, and operational control in PostgreSQL. The repository is organized as a Rust workspace composed of focused crates that together deliver the initial torrent + filesystem management minimal lovable product.

## Guiding Principles

-   **Single source of truth** – `DATABASE_URL` is the only required environment variable; all runtime configuration is stored in the database and hot-reloads across services.
-   **Composable crates** – Each domain area (configuration, API, engine, filesystem, telemetry) lives in its own crate with well-defined traits and DTOs.
-   **Observability first** – Telemetry, health, and structured events are wired in from the outset.
-   **Safe defaults** – Setup mode boots in a locked-down state until the operator unlocks the system through the API or CLI using a one-time token.

## Workspace Layout

```
revaer/
├─ Cargo.toml
├─ justfile
├─ README.md
├─ docs/
│  ├─ adr/
│  └─ api/
├─ config/
│  └─ reference-config.md        # Documentation only; no runtime reads
└─ crates/
   ├─ revaer-app                 # Composition root binary crate
   ├─ revaer-api                 # Axum HTTP + SSE services
   ├─ revaer-cli                 # Terminal client
   ├─ revaer-config              # DB-backed settings service
   ├─ revaer-fsops               # Filesystem post-processing
   ├─ revaer-telemetry           # Tracing, metrics, health hooks
   ├─ revaer-torrent-core        # Engine-agnostic traits & DTOs
   └─ revaer-torrent-libt        # libtorrent adapter
```

## Getting Started

1. Install the Rust toolchain (`rustup show`) at MSRV `1.91.0` (pinned in `rust-toolchain.toml`) and ensure `cargo`, `rustfmt`, and `clippy` are available.
2. Provide a PostgreSQL connection string via the `DATABASE_URL` environment variable.
3. Run `just check` to verify the workspace (all workflows must go through `just`; avoid calling `cargo …` directly).

## Development Tasks

-   `just fmt` – format sources.
-   `just lint` – run clippy with warnings as errors.
-   `just test` – execute the full test suite (integration + unit).
-   `just build` – build the workspace with all features enabled.
-   `just udeps` – detect unused dependencies (installs `cargo-udeps` on first run).
-   `just audit` – scan dependencies for published advisories (`cargo-audit`).
-   `just deny` – enforce the license and advisory policy (`cargo-deny`).
-   `just cov` – run source-based coverage with LLVM (requires `llvm-tools-preview`).
-   `just ci` – execute all required quality gates locally.
-   `just ui-e2e` – run Playwright API + UI E2E tests with temp databases and managed servers.

## UI E2E Testing

The Playwright suite lives under `tests/` and reads configuration from `tests/.env`. API checks (both auth modes) run before UI specs to catch backend failures first. Each run provisions a temp database.

1. Ensure ports `7070` and `8080` are free (the runner stops existing Revaer dev servers, but other services must be stopped manually).
2. Update `tests/.env` as needed (notably `E2E_DB_ADMIN_URL`, `E2E_BASE_URL`, `E2E_API_BASE_URL`, `E2E_FS_ROOT`).
3. Run `just ui-e2e`.

## Native Libtorrent Integration Test

The native libtorrent integration test suite is opt-in to keep default runs deterministic.

-   Enable it with `REVAER_NATIVE_IT=1`; it skips otherwise.
-   Ensure Docker is reachable (set `DOCKER_HOST` if not on `/var/run/docker.sock`).
-   Run `REVAER_NATIVE_IT=1 just ci` or `just test-native` when the native path should be covered (e.g., feature matrices).
-   See `docs/platform/native-tests.md` for the full setup and CI matrix note.

## CLI Tips

-   `revaer --output json ls` emits JSON suitable for scripting workflows (table output remains the default).
-   `revaer config get` and `revaer config set --file changes.json` provide a CLI wrapper around the `/v1/config` API so you can script updates without crafting HTTP requests manually.

## Optional OpenTelemetry Export

Set `REVAER_ENABLE_OTEL=true` to attach the (stubbed) OpenTelemetry layer. The exporter is disabled by default; when the flag is present the app uses `REVAER_OTEL_SERVICE_NAME` (defaults to `revaer-app`) and records the optional `REVAER_OTEL_EXPORTER` endpoint for future wiring. This keeps the instrumentation tree dormant unless you explicitly request it in environments that provide an OTLP collector.

### Required Tooling

Install the following Cargo subcommands before running the quality gates locally (the `just` recipes will also install them on first use):

```bash
cargo install cargo-udeps --locked
cargo install cargo-audit --locked
cargo install cargo-deny --locked
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
```

## Documentation

The documentation site is powered by [mdBook](https://rust-lang.github.io/mdBook/) and mirrors the automated docs pipeline for this repository.

1. Install the tooling once with `just docs-install` (installs `mdbook` and `mdbook-mermaid`).
2. Build and index the docs with `just docs` (runs `docs-build` followed by `docs-index`).
3. Preview locally via `just docs-serve`.

Pushes to `main` invoke the docs workflow to rebuild the book, refresh the LLM manifests under `docs/llm/`, and publish the static site to GitHub Pages.

## Next Steps

-   Implement the migrations and configuration schema inside `revaer-config`.
-   Wire the setup flow and runtime hot-reload between `revaer-app`, `revaer-api`, `revaer-torrent-*`, and `revaer-fsops`.
-   Author ADRs capturing architectural decisions, bootstrap flow, and configuration invariants.
