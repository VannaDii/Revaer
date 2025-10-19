# Revaer

Revaer is a data-driven media orchestration platform that centralizes configuration, telemetry, and operational control in PostgreSQL. The repository is organized as a Rust workspace composed of focused crates that together deliver the initial torrent + filesystem management minimal lovable product.

## Guiding Principles
- **Single source of truth** – `DATABASE_URL` is the only required environment variable; all runtime configuration is stored in the database and hot-reloads across services.
- **Composable crates** – Each domain area (configuration, API, engine, filesystem, telemetry) lives in its own crate with well-defined traits and DTOs.
- **Observability first** – Telemetry, health, and structured events are wired in from the outset.
- **Safe defaults** – Setup mode boots in a locked-down state until the operator unlocks the system through the API or CLI using a one-time token.

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
1. Install the Rust toolchain (`rustup show`) and ensure `cargo`, `rustfmt`, and `clippy` are available.
2. Provide a PostgreSQL connection string via the `DATABASE_URL` environment variable.
3. Run `cargo check` to verify the workspace.

## Development Tasks
- `just fmt` – format sources.
- `just lint` – run clippy with warnings as errors.
- `just test` – execute the full test suite (integration + unit).
- `just build` – build the workspace with all features enabled.
- `just udeps` – detect unused dependencies (installs `cargo-udeps` on first run).
- `just audit` – scan dependencies for published advisories (`cargo-audit`).
- `just deny` – enforce the license and advisory policy (`cargo-deny`).
- `just cov` – run source-based coverage with LLVM (requires `llvm-tools-preview`).
- `just ci` – execute all required quality gates locally.

### Required Tooling
Install the following Cargo subcommands before running the quality gates locally (the `just` recipes will also install them on first use):

```bash
cargo install cargo-udeps --locked
cargo install cargo-audit --locked
cargo install cargo-deny --locked
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
```

## Next Steps
- Implement the migrations and configuration schema inside `revaer-config`.
- Wire the setup flow and runtime hot-reload between `revaer-app`, `revaer-api`, `revaer-torrent-*`, and `revaer-fsops`.
- Author ADRs capturing architectural decisions, bootstrap flow, and configuration invariants.
