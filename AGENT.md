# AGENT.MD — Codex Operating Instructions (Revaer, Rust 2024)

> **Prime Directives**
>
> 1) **Rust 2024** only (never lower).  
> 2) **No dead code** (no unused items, no “future stubs”).  
> 3) **Minimal dependencies** (prefer `std`; add crates only with written rationale).  
> 4) **All operations via `just`**. CI **and** local dev MUST use the Justfile—never raw cargo in pipelines.
>
> **Completion Rule:** Because Codex runs locally, **a task is not complete** until **all requirements in this AGENT.MD are satisfied** and **all quality gates pass cleanly (no warnings, no errors)** via `just ci`.

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
    #![allow(clippy::module_name_repetitions)]
    ```
-   **Ban:** `#[allow(dead_code)]` anywhere. If you have unused items, delete them or feature-gate them behind code that is exercised in CI; do not leave “parking lot” code lying around.
-   **Ban:** `#[allow(clippy::too_many_lines)]` anywhere—split the code instead. Resolve the lint by extracting helpers that group related steps, moving reusable logic into private modules, or introducing small structs/impl blocks to own stateful behavior. Keep the original function as a thin orchestrator and add tests around the new pieces.
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

## 5) HTTP API & CLI

-   **API** (`revaer-api`): Axum, versioned under `/v1`. Apply Tower middleware for tracing, timeouts, compression, request size limits, and optional rate limiting.
-   **SSE** endpoints: must be cancellable, heartbeat at an interval, and obey client fan-out caps.
-   **OpenAPI**: deterministic export via `just api:export`; keep `docs/api/openapi.json` in sync; document examples on all routes and types.
-   **CLI** (`revaer-cli`): `--output json|table` (JSON is stable for scripting); idempotent reads; safe prompts (or `--yes`) for destructive actions; propagate correlation IDs.

---

## 6) Torrent Engine & FS Ops

-   **`revaer-torrent-core`**: domain logic, policies, selection rules, state machines; deterministic seeds where RNG is used.
-   **`revaer-torrent-libt`**: bindings/integration (feature-gated; isolate FFI; never leak unsafe into the rest of the codebase).
-   **`revaer-fsops`**: file/dir operations; non-blocking design in async contexts (use `spawn_blocking` when unavoidable, with explicit comments and tests).

---

## 7) Revaer Domain Rules

-   **File selection**: user-customizable glob filters; defaults are sensible (include common archives like `zip`, `rar`, `7z`, `tar.gz`, etc.; exclude junk). Users can reset to defaults.
-   **Seeding**: ratio/time goals; seed monitoring can re-start idle torrents when swarm health is low; scheduled bandwidth and torrent-count limits.
-   **Indexers**: trait abstraction; retries with jitter/backoff for idempotent ops only; normalized schema and result de-duplication.
-   **Media managers**: deterministic, explainable decisions (rationale fields in logs/spans).

---

## 8) Testing & Coverage

-   **Unit tests** per module (happy + edge cases). Use `#[cfg(test)]` for helpers instead of exporting them.
-   **Integration tests** in `/tests` (API, CLI, engine flows with mocks/fixtures); no real network by default.
-   **Property tests** (`proptest`) for parsers, schedulers, selection policies.
-   **Fuzz** (where applicable): torrent/magnet/file-pattern parsers.
-   **Determinism**: seeded RNGs; avoid flaky time-based assertions (use injected clocks).
-   **Coverage**: `cargo-llvm-cov` via `just cov`; libraries must meet **≥ 80%** coverage; **no regression** allowed.

---

## 9) Observability

-   **Spans** on all external boundaries: `http.request`, `engine.add_torrent`, `engine.tick`, `indexer.search`, `media.decide`.
-   **Span fields**: `request_id`, `torrent_id`, `indexer`, `decision_reason` (no PII; redact secrets).
-   **Metrics** (`revaer_*`):
    -   Counters: events (e.g., `revaer_engine_torrents_started_total`)
    -   Histograms: latencies (e.g., `revaer_indexer_query_latency_ms`)
    -   Gauges: active/queue sizes (e.g., `revaer_engine_active`)
-   Dev: human-readable logs; Prod: JSON logs. Both controlled via config.

---

## 10) Security

-   **Input validation** at all boundaries; body/response size limits; timeouts everywhere.
-   **Auth** extractors isolated; constant-time compares for tokens.
-   **HTTP security**: headers set; server banners disabled; minimize error leakage.
-   **Secrets**: env/OS store; never in repo; never logged (mask via tracing layer).
-   **Supply chain**: `just audit` and `just deny` must pass without exceptions—any temporary ignore must live in `.secignore` and be backed by an ADR with remediation recorded in `docs/adr/`.

---

## 11) Task & Review Rules (Local Codex, No PRs)

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

## 12) The Justfile Is Law

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
audit:        cargo audit --deny warnings --ignore-file .secignore
deny:         cargo deny check

# Build & test
build:        cargo build --workspace --all-features
build-release:cargo build --workspace --release --all-features
test:         cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features
cov:          cargo llvm-cov --workspace --fail-under 80

# API & docs
api-export:   cargo run -p revaer-api --bin generate_openapi

# Full CI gate
ci:           just fmt lint udeps audit deny test cov
```

-   If you introduce new feature flags, add a **feature matrix** section with additional `just` recipes (e.g., `test:feat[min]`, `test:feat[full]`) and have **both CI and local loops** call them.

---

## 13) CI (GitHub Actions) — must call `just`

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

## 14) How Codex Must Implement Work

1. Pick target crate/module (adhere to structure above).
2. Add a `/// # Design` rustdoc section on new modules, covering invariants and failure modes.
3. Write tests first (or alongside) so new code is **immediately used** (no dead items).
4. Implement minimal, clear code; avoid adding deps; if unavoidable, justify in the task record.
5. Add tracing/metrics; plumb config with validation.
6. Update OpenAPI/CLI/docs if surfaces change.
7. Run `just ci` locally; only then **mark the task complete**.

---

## 15) Style & Practical Tips

-   Prefer **newtype IDs** (`struct TorrentId(Uuid);`) and explicit constructors marked `#[must_use]`.
-   Keep modules cohesive (< ~300 LOC). Extract helpers.
-   Avoid premature generics; prefer concrete types until clarity demands abstraction.
-   Avoid blocking in async; if needed, `spawn_blocking` with clear rationale and tests.
-   Don’t leak types across crates without intent; enforce `pub(crate)` by default.
-   Use `Result<T, E>` with domain-specific `E`; avoid boolean flags that hide errors.

---

_This document is normative. If code and AGENT.MD disagree, update the code to comply or add an ADR and amend AGENT.MD with rationale._
