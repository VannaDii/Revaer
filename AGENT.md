# AGENT.MD — Codex Operating Instructions (Revaer, Rust 2024)

> **Prime Directives**
>
> 1. **Rust 2024** only (never lower).
> 2. **No dead code** (no unused items, no “future stubs”).
> 3. **Minimal dependencies** (prefer `std`; add crates only with written rationale).
> 4. **All operations via `just`**. CI **and** local dev MUST use the Justfile—never raw cargo in pipelines.
> 5. **Stored procedures or bust**: every runtime database interaction is executed via stored procedures; inline SQL is only allowed inside migrations.
> 6. **`just ci` before every hand-off**: run the full pipeline locally and fix failures before you declare a task done.
> 7. **Dependencies are injected**: runtime logic receives collaborators from callers (traits/fakes allowed); only bootstrap/wiring code may construct concrete impls or read the environment.
>
> **Completion Rule:** Because Codex runs locally, **a task is not complete** until **all requirements in this AGENT.MD are satisfied** and **`just ci` passes cleanly (no warnings, no errors)**.

---

## 0) Repository Shape (use & extend these patterns)

```
Cargo.toml          # workspace manifest (rust-version, profiles)
Cargo.lock
justfile            # the single interface for build/test/release/etc.
README.md
rust-toolchain.toml # pinned stable channel; edition 2024 in all crates
codealike.json

config/             # samples, docs
crates/
  revaer-api/               src/
  revaer-app/               src/           # thin bin: wires libs
  revaer-cli/               src/
  revaer-config/            {migrations,src,tests}
  revaer-events/            src/
  revaer-fsops/             src/
  revaer-telemetry/         src/
  revaer-torrent-core/      src/
  revaer-torrent-libt/      src/
docs/
  adr/                      # architecture decision records
  api/
    guides/
  phase-one-roadmap.md
target/              # build artifacts

.git/
.github/workflows/
.vscode/
```

-   **Library-first:** `revaer-app` is a thin binary; all logic lives in libraries under `crates/`.
-   Keep **public API small** and **`pub(crate)` by default**. Only expose what’s needed across crates.
-   New subsystems must follow the same pattern (crate-per-domain, small public surface).

---

## 1) Language & Compiler Gates (enforced in every crate)

-   **Edition:** `2024` (crate `Cargo.toml`: `edition = "2024"`). **Never lower this.**
-   **MSRV:** pinned via `rust-toolchain.toml` and `package.rust-version` in the workspace.
-   Top-level crate attributes (add to each `lib.rs`/`main.rs`):
    ```rust
    #![forbid(unsafe_code)]
    #![deny(
        warnings,
        dead_code,
        unused,
        unused_imports,
        unused_must_use,
        unreachable_pub,
        clippy::all,
        clippy::pedantic,
        clippy::cargo,
        clippy::nursery,
        rustdoc::broken_intra_doc_links,
        rustdoc::bare_urls,
        missing_docs
    )]
    ```
-   **Ban:** `#[allow(dead_code)]`, `#[allow(missing_docs)]`, `#[allow(clippy::cast_precision_loss)]`,`#[allow(clippy::cast_sign_loss)]`, `#[allow(clippy::missing_const_for_fn)]`, `#[allow(clippy::cast_possible_truncation)]`, `#[allow(clippy::missing_errors_doc)]`, `#[allow(clippy::non_send_fields_in_send_ty)]` anywhere. If you have unused items, delete them or feature-gate them behind code that is exercised in CI; do not leave “parking lot” code lying around.
    -   Exceptions: The minimal and necessary `#[allow(...)]` code can only be used in FFI interactions that cannot be accomplished in Rust or thru an existing crate.
-   **Ban:** `#[allow(clippy::too_many_lines)]` anywhere—split the code instead. Resolve the lint by extracting helpers that group related steps, moving reusable logic into private modules, or introducing small structs/impl blocks to own stateful behavior. Keep the original function as a thin orchestrator and add tests around the new pieces.
-   **Ban:** crate-level allowances for `clippy::module_name_repetitions`, `unexpected_cfgs`, and `clippy::multiple_crate_versions`. Design names and dependency graphs so these lints pass without suppressions.
-   Dependency hygiene is enforced with `cargo-deny` and `cargo-udeps`; `clippy::cargo` is no longer part of the crate-level lint set.
-   Mark important return values `#[must_use]` (IDs, handles, results with side effects).
-   Deny `unreachable_pub` to prevent unused public API leakage across crates.

---

## 2) Dead Code Policy (absolute)

-   If code is merged, it’s **used**: in production paths or tests behind `#[cfg(test)]` or feature gates that are exercised by CI.
-   No “parking lots”—design ideas go in `docs/adr/`; **unused code is removed**.
-   Feature-gated items MUST be exercised by **both** local and CI feature matrices; otherwise remove them.
-   `cargo-udeps` (via `just udeps`) must be **clean**—no unused dependencies or targets.

---

## 3) Dependencies (minimalism)

-   Prefer **`std`** and small, well-maintained crates. Add a dependency **only** with a concise written rationale in the **task record**:
    -   “why this, why now, and the alternatives considered.”
-   Canonical choices (only if needed; keep feature sets minimal):
    -   Async runtime: **Tokio** (single runtime; opt-in features only).
    -   HTTP/API: **Axum**, **tower**, **tower-http** (selective features).
    -   Serde: **serde**, **serde_json** (derive only).
    -   Errors: **thiserror** (libs); **anyhow** allowed only in bins/tests.
    -   Tracing: **tracing**, **tracing-subscriber** (fmt/json via features).
-   Avoid heavy transitive trees (templating engines, ORMs, giant utility crates); prioritize **explicit code** and **narrow helpers**.
-   License and security policy is enforced by `just deny` and `just audit`.

---

## 4) Configuration, Telemetry, Events

-   **`revaer-config`**: strongly typed config structs; load order = defaults → file → env → CLI. Validate at load; log **effective config** with secrets redacted (dev only human-readable; prod JSON).
-   **`revaer-telemetry`**: single init for tracing, structured logs, and metrics. Prod defaults JSON logs; dev defaults pretty logs. Include log level, target, and correlation IDs.
-   **`revaer-events`**: internal event types for decoupled subsystems—document topic names, payload schema, and cardinality considerations. Avoid dynamic strings for event kinds—use enums.

---

## 5) Database Access (stored procedures only)

-   **Runtime code never embeds SQL**—all `sqlx::query` calls must wrap stored procedure invocations (e.g., `SELECT revaer_config.apply_patch($1, $2)`).
-   **Parameter binding & Transactions**: Use Named Bind Parameters and Transactions for Consistency and Safety in accordance with https://sqlx.dev/article/Best_Practices_for_Writing_SQLX_Code.html
-   **Inline SQL is migration-only**: schema definitions, stored procedure bodies, and seed data live under `crates/*/migrations/`. No other crate may ship raw DML/DDL text.
-   **Versioned procedures**: every behavioural change ships as a migration that updates the procedure(s) and bumps the revision.
-   **Shared access**: when multiple crates touch the same DB state, they all call the same stored procedure(s); don’t duplicate logic per crate.
-   **Tests follow suit**: unit/integration tests exercise behaviour through the stored procedure APIs so coverage stays representative.

---

## 6) HTTP API & CLI

-   **API** (`revaer-api`): Axum, versioned under `/v1`. Apply Tower middleware for tracing, timeouts, compression, request size limits, and optional rate limiting.
-   **SSE** endpoints: must be cancellable, heartbeat at an interval, and obey client fan-out caps.
-   **OpenAPI**: deterministic export via `just api:export`; keep `docs/api/openapi.json` in sync; document examples on all routes and types.
-   **CLI** (`revaer-cli`): `--output json|table` (JSON is stable for scripting); idempotent reads; safe prompts (or `--yes`) for destructive actions; propagate correlation IDs.

---

## 7) Torrent Engine & FS Ops

-   **`revaer-torrent-core`**: domain logic, policies, selection rules, state machines; deterministic seeds where RNG is used.
-   **`revaer-torrent-libt`**: bindings/integration (feature-gated; isolate FFI; never leak unsafe into the rest of the codebase).
-   **`revaer-fsops`**: file/dir operations; non-blocking design in async contexts (use `spawn_blocking` when unavoidable, with explicit comments and tests).

---

## 8) Revaer Domain Rules

-   **File selection**: user-customizable glob filters; defaults are sensible (include common archives like `zip`, `rar`, `7z`, `tar.gz`, etc.; exclude junk). Users can reset to defaults.
-   **Seeding**: ratio/time goals; seed monitoring can re-start idle torrents when swarm health is low; scheduled bandwidth and torrent-count limits.
-   **Indexers**: trait abstraction; retries with jitter/backoff for idempotent ops only; normalized schema and result de-duplication.
-   **Media managers**: deterministic, explainable decisions (rationale fields in logs/spans).

---

## 9) Testing & Coverage

-   **Unit tests** per module (happy + edge cases). Use `#[cfg(test)]` for helpers instead of exporting them.
-   **Integration tests** in `/tests` (API, CLI, engine flows with mocks/fixtures); no real network by default.
-   **Property tests** (`proptest`) for parsers, schedulers, selection policies.
-   **Fuzz** (where applicable): torrent/magnet/file-pattern parsers.
-   **Determinism**: seeded RNGs; avoid flaky time-based assertions (use injected clocks).
-   **Coverage**: `cargo-llvm-cov` via `just cov`; libraries must meet **≥ 80%** coverage; **no regression** allowed.
-   **All crates are covered**: the workspace coverage gate (80% line) applies to every crate, including new ones. If a crate ships a binary, put logic in `lib` and test it to satisfy the gate; no exemptions.
-   **No coverage suppression**: never pass `--ignore-filename-regex`, `--ignore-run-fail`, `--no-report`, `--summary-only`, target filters, or any other `cargo llvm-cov` option that hides code from analysis. If the gate fails, add tests or remove code—do not suppress it.

---

## 10) Observability

-   **Spans** on all external boundaries: `http.request`, `engine.add_torrent`, `engine.tick`, `indexer.search`, `media.decide`.
-   **Span fields**: `request_id`, `torrent_id`, `indexer`, `decision_reason` (no PII; redact secrets).
-   **Metrics** (`revaer_*`):
    -   Counters: events (e.g., `revaer_engine_torrents_started_total`)
    -   Histograms: latencies (e.g., `revaer_indexer_query_latency_ms`)
    -   Gauges: active/queue sizes (e.g., `revaer_engine_active`)
-   Dev: human-readable logs; Prod: JSON logs. Both controlled via config.

---

## 11) Security

-   **Input validation** at all boundaries; body/response size limits; timeouts everywhere.
-   **Auth** extractors isolated; constant-time compares for tokens.
-   **HTTP security**: headers set; server banners disabled; minimize error leakage.
-   **Secrets**: env/OS store; never in repo; never logged (mask via tracing layer).
-   **Supply chain**: `just audit` and `just deny` must pass without exceptions—any temporary ignore must live in `.secignore` and be backed by an ADR with remediation recorded in `docs/adr/`.

---

## 12) Task & Review Rules (Local Codex, No PRs)

Since Codex runs locally, **a task is not complete** until **all requirements in this AGENT.MD are satisfied** and **all quality gates pass cleanly (no warnings, no errors)** via `just ci`.

-   **Conventional Commits (optional local log):** keep a concise local change log using conventional-style entries to aid future OSS history.
-   **Task Record Must Include:** Motivation, Design notes, Test coverage summary, Observability updates, Risk & rollback plan, and **Dependency rationale** (if any) with alternatives considered.
-   **No ad-hoc env overrides:** enforce warnings-as-errors through `cargo --config 'build.rustflags=["-Dwarnings"]' …`; never hide commands with `@` or depend on transient `RUSTFLAGS`.
-   **Local Review Loop (ignore README.md as authoritative context; always keep README.md in sync with changes):**

    1. Run `just fmt` and `just lint` (no warnings allowed).
    2. Run `just udeps` (must be clean; unused dependencies are disallowed).
    3. Run `just test` and `just cov` (≥ 80% lib coverage; no regressions).
    4. Run `just audit` and `just deny` (must pass cleanly; any temporary advisory ignores belong in `.secignore` and require an ADR with remediation steps).
    5. Run `just build-release` to ensure release readiness.

-   **Checklist (must all be true before marking the task complete):**
    -   [ ] No `unsafe`, no `unwrap/expect` (outside tests/examples)
    -   [ ] **Zero dead code**; `just udeps` clean; `unreachable_pub` denied
    -   [ ] Edition **2024**; builds with `-D warnings` across all crates
    -   [ ] Minimal dependency footprint; feature flags minimized; any new deps justified in the task record
    -   [ ] Tracing/metrics added where relevant; sensitive data redacted
    -   [ ] Config validated at load; effective config logged (secrets redacted)
    -   [ ] OpenAPI/CLI/help/docs updated if surfaces changed
    -   [ ] Coverage ≥ 80% (libs), **no coverage regression**
    -   [ ] All `just` gates (`just ci`) pass **without warnings or errors**

**Completion Rule:** Do **not** declare a task complete or exit until all above checks pass. Persist the task record alongside code changes (e.g., `docs/adr/NNN-task.md`).

-   Use `docs/adr/template.md` as the starting point for new ADRs—copy it with standard shell tooling (`cp`, `mv`) and number the file sequentially.

---

## 13) The Justfile Is Law

All ops—local and CI—MUST run through these recipes (names are normative).

> **Do not** add raw `cargo` invocations to CI. If a new step is needed, add a `just` recipe and call it in CI **and** local runs.

### Required `just` recipes (canonical names)

```make
# Bootstrap & hygiene
fmt:          cargo fmt --all --check
fmt-fix:      cargo fmt --all
lint:         cargo clippy --workspace --all-targets --all-features -- -D warnings
check:        cargo --config 'build.rustflags=["-Dwarnings"]' check --workspace --all-targets --all-features
udeps:        cargo +stable udeps --workspace --all-targets
audit:        ignore_args=""; \
              if [ -f .secignore ]; then \
                  while IFS= read -r advisory; do \
                      case "$advisory" in \
                          \#*|"") ;; \
                          *) ignore_args="$ignore_args --ignore $advisory" ;; \
                      esac; \
                  done < .secignore; \
              fi; \
              cargo audit --deny warnings $ignore_args
deny:         cargo deny check

# Build & test
build:        cargo build --workspace --all-features
build-release:cargo build --workspace --release --all-features
test:         cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features
cov:          cargo llvm-cov --workspace --fail-under-lines 80

# API & docs
api-export:   cargo run -p revaer-api --bin generate_openapi

# Full CI gate
ci:           just fmt lint udeps audit deny test cov
```

-   If you introduce new feature flags, add a **feature matrix** section with additional `just` recipes (e.g., `test:feat[min]`, `test:feat[full]`) and have **both CI and local loops** call them.

---

## 14) CI (GitHub Actions) — must call `just`

Required jobs (fail-fast):

1. **fmt** → `just fmt`
2. **lint** → `just lint`
3. **udeps** → `just udeps`
4. **supply-chain** → `just audit` and `just deny`
5. **test** → `just test`
6. **coverage** → `just cov`
7. **build-release** (tags/main) → `just build-release` and publish artifacts (binaries, OpenAPI, SBOM/license report)
8. **feature-matrix** → run additional `just test:feat[...]` if features exist
9. **msrv** (if distinct from stable) → call the same `just` targets using the MSRV toolchain

> CI must never run `cargo …` directly—only `just …`.

---

## 15) How Codex Must Implement Work

1. Pick target crate/module (adhere to structure above).
2. Add a `/// # Design` rustdoc section on new modules, covering invariants and failure modes.
3. Write tests first (or alongside) so new code is **immediately used** (no dead items).
4. Implement minimal, clear code; avoid adding deps; if unavoidable, justify in the task record.
5. Add tracing/metrics; plumb config with validation.
6. Update OpenAPI/CLI/docs if surfaces change.
7. Run `just ci` locally; only then **mark the task complete**.

---

## 16) Style & Practical Tips

-   Prefer **newtype IDs** (`struct TorrentId(Uuid);`) and explicit constructors marked `#[must_use]`.
-   Keep modules cohesive (< ~300 LOC). Extract helpers.
-   Avoid premature generics; prefer concrete types until clarity demands abstraction.
-   Avoid blocking in async; if needed, `spawn_blocking` with clear rationale and tests.
-   Don’t leak types across crates without intent; enforce `pub(crate)` by default.
-   Use `Result<T, E>` with domain-specific `E`; avoid boolean flags that hide errors.

---

## 17) Frontend (`crates/revaer-ui`) organization (retroactive and forward)

-   Layout (normalize existing files to match; no new grab-bags at crate root):

```text
crates/revaer-ui/src/
  app/{mod.rs,routes.rs,preferences.rs,storage.rs,sse.rs}
  core/{ui.rs,breakpoints.rs,theme.rs,i18n/,logic/{layout.rs,shortcuts.rs,format.rs,virtual_list.rs}}
  services/{api.rs,sse.rs}
  features/
    torrents/{mod.rs,view.rs,state.rs,actions.rs,logic.rs,api.rs}
    dashboard/{mod.rs,view.rs,state.rs,api.rs}
    ... (one folder per route/vertical slice)
  components/   # shared UI atoms/composites only
  models.rs     # transport DTOs only
  main.rs       # wasm entry stub
  lib.rs        # exports/re-exports of core primitives
```

-   `app/*` is the only place that touches `window`, `LocalStorage`, `EventSource`, router, and context providers. `preferences.rs` owns persistence keys + `api_base_url` remapping; `sse.rs` handles EventSource + backoff + dispatch to reducers.
-   `core/*` is DOM-free and testable on the host; keep UI primitives (`UiMode`, `Density`, `Pane`), breakpoints, theme tokens, i18n data, and pure logic helpers there.
-   `services/*` is transport-only (REST + SSE); no Yew/gloo/web-sys. Convert DTOs to feature state before returning.
-   `features/*` are vertical slices. Each owns `state.rs` (view state + reducers, pure), `actions.rs` (enums/toast text), `logic.rs` (per-feature helpers), `api.rs` (calls into services), `view.rs` (Yew components), `mod.rs` (re-exports). Features do not reach into each other directly; they depend on `core`, `components`, and `services`.
-   `components/*` hosts shared UI pieces (AppShell, ToastHost, SseOverlay, virtual list). No persistence, API calls, or SSE side effects inside components; data flows via props/callbacks.
-   `models.rs` holds transport DTOs only. UI-only fields live in feature state. Keep conversions in `services` or feature `state.rs`.
-   Retroactive mandate: migrate existing `app.rs`, `logic.rs`, `state.rs`, and other root-level helpers into the structure above. New code must align with this layout; deviations require ADR approval.

---

## 18) Crate archetypes & layout (retroactive; applies to all crates)

Pick the matching archetype and align existing crates; no grab-bag modules at root. Each crate’s `main.rs` (if any) must be a thin bootstrap that defers to `lib.rs`.

---

## 19) File & Module Organization (small, cohesive units)

The goal is that **no single file becomes a grab-bag**. Files must stay **small, cohesive, and named for what they own**. If you’re scrolling forever or adding unrelated types “because they’re nearby,” you’re breaking this rule.

### 19.1 General rules

-   **Single responsibility per file**

    -   Each `*.rs` file must have **one primary responsibility** along a clear axis:
        -   by layer (`domain`, `app`, `http`, `infra`, `tasks`, `telemetry`, `config`, etc.), or
        -   by feature/vertical (`torrents`, `setup`, `dashboard`, `indexers`), or
        -   by type kind (`requests`, `responses`, `errors`, `extractors`, `rate_limit`, etc.).
    -   If you can’t summarize the file in a single sentence without “and also,” it’s probably doing too much.

-   **File size guidance**

    -   Target **≤ ~300–400 non-test LOC per file** for production code.
    -   Hitting `clippy::too_many_lines` is treated as a **design smell**, not a lint to be silenced. Fix it by:
        -   extracting helpers into private functions,
        -   moving cohesive logic into a dedicated module, or
        -   splitting the file along a clear responsibility boundary.
    -   Test modules may be larger, but if a single `tests` module starts to sprawl, split into `mod something_tests;` files under `tests/`.

-   **Naming must reflect contents**
    -   A file name must clearly describe what it owns. Some canonical patterns:
        -   `api.rs` – API trait / router wiring for a feature or service.
        -   `requests.rs` / `responses.rs` – transport DTOs for HTTP.
        -   `errors.rs` – error enums/types for that module.
        -   `state.rs` – module-local state structs, not global grab-bags.
        -   `auth.rs`, `rate_limit.rs`, `sse.rs`, `health.rs` – behaviorally scoped modules.
    -   If someone can’t guess what’s inside from the filename, rename or split it.

### 19.2 Types-per-file rules (“like-kind” only)

-   **Allowed:** multiple types of the same “kind” in a single file when the name reflects that:

    -   `responses.rs` may contain all HTTP response shapes for a given area:
        -   `DashboardResponse`, `HealthResponse`, `FullHealthResponse`, etc.
    -   `errors.rs` may hold `ApiError`, `DomainError`, helper structs like `ErrorRateLimitContext`, as long as they are **error-centric and local** to that module.
    -   `rate_limit.rs` may hold `RateLimiter`, `RateLimitStatus`, `RateLimitSnapshot`, `RateLimitError`, plus helpers.

-   **Not allowed:** mixing unrelated kinds in one file:

    -   Do **not** define API traits, HTTP handlers, router construction, DTOs, auth extractors, rate limiting, OpenAPI persistence, and test harnesses all in a single `lib.rs` or `api.rs`.
    -   Do **not** put domain types, HTTP DTOs, and infra adapters in one file “for convenience.”
    -   If two types **would normally live in different folders** (`domain/`, `http/`, `infra/`, `telemetry/`), they must **not** share a file.

-   **Like-kind rule of thumb**
    -   If all types would be described with the same suffix in docs (“…response types”, “…request types”, “…rate limiting primitives”, “…auth extractors”), they can share a file.
    -   If you’d naturally split the sentence (“this file has API traits, response types, and the whole router”), it must be split.

### 19.3 `lib.rs` and `main.rs` constraints

-   **`lib.rs`**

    -   `lib.rs` is for:
        -   crate docs (`//!`),
        -   `pub mod` declarations,
        -   **light** re-exports, and
        -   very small glue types (simple newtypes, marker traits) that truly represent the crate boundary.
    -   `lib.rs` must **not**:
        -   contain full API implementations,
        -   define large structs with behavior,
        -   host HTTP handlers, routers, or Axum/Tower wiring,
        -   embed rate limiting logic, event streaming, or complex state structs.
    -   Any non-trivial behavior seen in `lib.rs` must be moved to an appropriately named module:
        -   e.g. `http/api_server.rs`, `http/state.rs`, `http/auth.rs`, `http/rate_limit.rs`, `http/sse.rs`, `http/health.rs`, `http/openapi.rs`, etc., with `lib.rs` re-exporting as needed.

-   **`main.rs`**
    -   `main.rs` remains a **thin bootstrap only**:
        -   parse config/CLI,
        -   initialize telemetry,
        -   wire concrete implementations,
        -   call a `run()` in `bootstrap.rs` or equivalent.
    -   No business logic, no HTTP handlers, no domain types in `main.rs`.

### 19.4 Module hierarchy & layering

-   **Respect crate archetypes (Section 18)** at the directory level and mirror that at the file level:

    -   `http/`:
        -   `router.rs` – route wiring.
        -   `handlers/` – one file per feature/vertical (e.g. `torrents.rs`, `setup.rs`, `health.rs`, `dashboard.rs`).
        -   `dto/` – `requests.rs`, `responses.rs`, `errors.rs`.
        -   `auth.rs`, `rate_limit.rs`, `sse.rs`, `middleware.rs` – cross-cutting concerns.
    -   `app/`:
        -   `services/` – orchestration per use-case (`torrents_service.rs`, `setup_service.rs`).
        -   No HTTP or transport types here; operate on domain types.
    -   `domain/`:
        -   `model/` – core types per concept (`torrent.rs`, `config.rs`).
        -   `policy/` – rules/decisions in dedicated files.
        -   `service/` – pure services per domain concern.

-   **Tests and support code**
    -   Unit tests local to a module live in the same file in `#[cfg(test)] mod tests { … }`, or in a dedicated `modname_tests.rs` if they get large.
    -   Shared test helpers belong in a test support crate (`revaer-test-support`) or clearly named `tests/fixtures.rs`, **never** in production modules.

### 19.5 Refactoring triggers (when you MUST split a file)

You **must** split or reorganize a file when any of the following are true:

1. Clippy complains about `too_many_lines` and you’re tempted to silence it.
2. The file defines:
    - a long-lived state struct (`ApiState`) **and**
    - a server wrapper (`ApiServer`) **and**
    - HTTP handlers **and**
    - middleware **and/or**
    - DTOs and helper types.
3. A reviewer or your future self struggles to find where a given behaviour lives (“where is rate limiting implemented?”, “where is SSE filtered?”).
4. You find yourself using comments like `// region: X` to mentally group sections — each “region” probably deserves a module.

At each trigger, **split by responsibility** and ensure file names and paths reflect the new structure. Update `lib.rs`/`mod.rs` docs to describe the layout after the change.

_This section is normative. If a file organization choice conflicts with 19.x, reorganize the code to comply rather than weakening lints or adding grab-bag files._

---

### Service/daemon crates (`revaer-api`, `revaer-runtime`, `revaer-doc-indexer`)

```
src/
  main.rs       # thin: parse config, call bootstrap
  lib.rs        # re-exports + crate docs
  bootstrap.rs  # wire config, telemetry, infra, router/workers
  config/       # typed config + validation (no IO side effects)
  domain/       # pure domain models/policies (no IO)
  app/          # use-cases/services orchestrating domain + infra
  http/         # router.rs, routes.rs, handlers/, dto/, extractors/, middleware/
  infra/        # adapters: db repos, external clients, storage, queues, cache
  tasks/        # background jobs/cron/schedulers (no HTTP handlers)
  telemetry/    # crate-scoped metrics/tracing helpers (rely on revaer-telemetry)
```

-   `http` owns request/response DTOs; domain stays JSON-free.
-   `app` uses interfaces defined in `domain` and implemented in `infra`; no direct DB calls from handlers.

### CLI crate (`revaer-cli`)

```
src/
  main.rs     # thin: parse CLI, invoke commands
  lib.rs
  cli.rs      # clap args + validation
  commands/   # one file per subcommand (pure orchestration)
  client.rs   # API client wrapper
  output.rs   # renderers (table/json), no network
  config.rs   # CLI config loading/merging (reuse revaer-config types)
```

-   Commands call `client.rs`/`app` helpers; rendering isolated in `output.rs`.

### Config crate (`revaer-config`)

```
src/
  lib.rs
  model.rs     # typed config structs
  defaults.rs
  loader.rs    # file/env/cli merge (no globals)
  validate.rs
```

-   No runtime state; pure data + validation.

### Data/migrations crate (`revaer-data`)

```
src/
  lib.rs
  config.rs
  runtime.rs      # migration runner/stores facade
migrations/       # SQL/procs only
tests/            # integration tests hitting migrator/stores
```

-   Runtime code calls stored procedures only; no inline SQL outside migrations.

### Telemetry crate (`revaer-telemetry`)

```
src/
  lib.rs
  init.rs       # setup tracing/metrics/logging
  filters.rs
  layers.rs
  context.rs    # request/task scoped IDs, redaction helpers
  metrics.rs    # metric registrations/helpers
```

-   No business logic; only observability primitives consumed by other crates.

### Events crate (`revaer-events`)

```
src/
  lib.rs
  topics.rs     # topic/channel names
  payloads.rs   # event structs/enums (serde)
  routing.rs    # helper traits for producers/consumers
```

-   Pure types + helpers; no transport clients here.

### Domain/engine crates (`revaer-torrent-core`, `revaer-fsops`)

```
src/
  lib.rs
  model/        # core types/newtypes
  policy/       # rules/decisions
  service/      # pure services/use-cases (no IO)
  planner/      # schedulers/strategies (pure)
  adapters/     # optional abstractions for IO implemented elsewhere
```

-   Keep them IO-free; external effects belong to callers/adapters in infra crates.

### FFI/integration crate (`revaer-torrent-libt`)

```
src/
  lib.rs
  ffi.rs        # unsafe boundary isolated here
  types.rs      # translated types/newtypes
  adapter.rs    # safe wrappers around FFI calls
  convert.rs    # mapping between FFI and domain types
```

-   Unsafe contained to `ffi.rs`; public surface is safe wrappers.

### Runtime/support crates (`revaer-runtime`, `revaer-test-support`)

-   `revaer-runtime`: follow Service layout; background workers/schedulers live under `tasks/`; runtime wiring in `bootstrap.rs`.
-   `revaer-test-support`: helpers/fixtures only. `src/{lib.rs,fixtures.rs,mocks.rs,assert.rs}`; no network/DB side effects by default (use traits/injected clients for fakes).

### Cross-cutting rules

-   No new root-level catch-all modules (`utils`, `helpers`, `logic`) in any crate. Place code in the archetype folders above.
-   Retroactive mandate: reorganize existing crates to match; deviations require an ADR with rationale.
-   Keep domain modules pure; IO and side effects live in `infra`, `http`, or `tasks` as appropriate.
-   Each crate documents its module layout in `lib.rs` rustdoc (one paragraph, updated with structure changes).
-   New crates (forward-looking rules):
    -   Choose an archetype above before creating files; note the choice + rationale in an ADR.
    -   Scaffold the directory tree up front (empty modules with `// TODO` are not allowed; add minimal code + tests or omit the file).
    -   No `utils.rs`/`helpers.rs`/`misc.rs` in new crates; place code in the archetype folders.
    -   Keep `pub(crate)` by default; expose only the minimal API needed by dependants.
    -   Add crate docs in `lib.rs` describing its purpose and chosen archetype; update when structure changes.
    -   Wire new crates into `just`/CI if they add binaries, migrations, or feature flags.

---

_This document is normative. If code and AGENT.MD disagree, update the code to comply._
