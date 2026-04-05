# Revaer Remediation Plan

This checklist is based on current code and documentation inspection as of 2026-04-04. It is intended to track verified implementation and process gaps to closure without copying stale roadmap items forward uncritically.

Closeout status: the Phase One remediation work tracked here has been implemented and re-verified against the repo. Items remain in this document as an audit trail, not because additional gaps are still open.

## Current Findings

### [x] Dashboard API is no longer stubbed

Current status: `/v1/dashboard` now aggregates live torrent rates/counts from injected runtime state and falls back safely when the snapshot cannot be collected.

Evidence:

- `crates/revaer-api/src/http/handlers/health.rs`
- `crates/revaer-api/src/app/state.rs`
- `docs/phase-one-roadmap.md#L14-L17`

Remediation checklist:

- [x] Define a dashboard snapshot source of truth owned by runtime state rather than the handler.
- [x] Source transfer rates, torrent counts, and disk usage from live collaborators already injected into the API layer.
- [x] Replace the hard-coded zero payload in the dashboard handler with the live snapshot.
- [x] Define explicit fallback behavior when backing state is unavailable and return safe values without silently masking failure causes.
- [x] Add API tests that prove non-stub behavior when runtime state is populated.
- [x] Add API tests that prove the fallback path when snapshot dependencies are unavailable or degraded.

Done when:

- [x] `/v1/dashboard` reflects live values from runtime state.
- [x] The handler no longer contains hard-coded metric placeholders.
- [x] Tests cover both healthy and degraded snapshot sourcing behavior.

### [x] FsOps archive, PAR2, and checksum support is complete

Current status: the FsOps pipeline now supports the intended Phase One archive matrix, honors configured PAR2 policy at runtime, and records deterministic checksum metadata alongside `.revaer.meta`.

Evidence:

- `crates/revaer-fsops/src/service/mod.rs`
- `crates/revaer-fsops/src/error.rs`
- `crates/revaer-config/src/loader.rs`
- `docs/phase-one-roadmap.md#L13-L13`

Remediation checklist:

- [x] Expand archive handling beyond `zip` to the supported format set agreed for Phase One.
- [x] Decide whether archive tooling is library-backed, command-backed, or mixed, and document the rationale in the implementation task record.
- [x] Implement PAR2 verify and repair stages that honor the configured `FsPolicy.par2` mode.
- [x] Record checksum metadata alongside `.revaer.meta` when cleanup or validation behavior depends on file identity.
- [x] Document and implement non-Unix fallback behavior for chmod/chown/umask handling if portability remains a requirement.
- [x] Add FsOps tests that cover supported archive formats, PAR2 decisions, checksum persistence, and fallback behavior.

Done when:

- [x] The pipeline supports the intended archive matrix instead of failing for every non-zip archive.
- [x] PAR2 policy produces observable runtime behavior rather than config-only state.
- [x] Checksum handling is implemented with tests and reflected in operator-facing docs.

### [x] qBittorrent compatibility has its planned Phase One surface

Current status: the compatibility router now covers the planned Phase One mutation surface for rename, relocate, category/tag changes, reannounce, and recheck, with the supported scope documented in code.

Evidence:

- `crates/revaer-api/src/http/handlers/compat_qb.rs`
- `docs/phase-one-roadmap.md#L14-L14`

Remediation checklist:

- [x] Define the intended qBittorrent parity scope so compatibility has a bounded, testable target.
- [x] Add the missing high-value mutation endpoints, including rename, relocate, category/tag mutation, recheck, and reannounce if those remain in scope.
- [x] Reuse existing domain services rather than duplicating mutation logic inside the compatibility layer.
- [x] Add compatibility tests for each supported route, including request shape, auth behavior, and response semantics.
- [x] Update the roadmap and API docs to describe exactly which qB endpoints are supported and which are intentionally out of scope.

Done when:

- [x] Supported qB endpoints are explicitly documented.
- [x] The highest-priority missing mutation routes are implemented and tested.
- [x] Remaining omissions are deliberate and documented rather than accidental.

### [x] OpenTelemetry export is no longer placeholder-level

Current status: OTEL export now wires a real OTLP tracing exporter behind explicit configuration, remains dormant when disabled, and is enabled in the app build that consumes telemetry.

Evidence:

- `README.md#L77-L79`
- `crates/revaer-telemetry/src/init.rs`
- `crates/revaer-app/src/bootstrap.rs`

Remediation checklist:

- [x] Replace placeholder OTEL wiring with a real exporter path that is enabled only when OTEL configuration is explicitly turned on.
- [x] Preserve the current dormant-by-default behavior when OTEL is disabled.
- [x] Validate configuration early and surface actionable startup errors for invalid exporter settings.
- [x] Add smoke tests or focused unit tests for OTEL disabled and OTEL enabled initialization flows.
- [x] Update README and operator docs to describe the actual supported OTEL configuration surface.

Done when:

- [x] OTEL configuration drives a real exporter path rather than storing an unused endpoint.
- [x] Disabled OTEL remains a no-op.
- [x] Invalid OTEL configuration fails early with structured startup errors.
- [x] Tests cover enabled and disabled initialization behavior.

### [x] Operational validation is automated where feasible and explicitly manual where required

Current status: a checked-in `just runbook` automation path now wraps the repeatable API/UI validation flow, captures artifacts, and leaves only deployment-specific fault-injection drills as documented manual checks.

Evidence:

- `docs/runbook.md#L1-L53`
- `scripts/dev-seed.sql`
- `docs/phase-one-roadmap.md#L18-L18`

Remediation checklist:

- [x] Convert the runbook’s critical validation steps into one or more `just` recipes that can be executed locally and in CI where feasible.
- [x] Keep `docs/runbook.md` as the human-readable operator guide, but make it reference the automation commands instead of duplicating manual steps blindly.
- [x] Define the expected artifacts for successful operational validation, including health snapshots, metrics captures, and event traces where appropriate.
- [x] Add clear prerequisites and failure output so the automated runbook is actionable when it breaks.
- [x] Decide which scenarios remain manual-only and document why.

Done when:

- [x] The key runbook scenarios are executable through `just` recipes or another checked-in automation entrypoint.
- [x] Manual steps are limited to cases that truly cannot be automated.
- [x] The runbook points to a repeatable validation flow instead of serving as the only execution path.

### [x] Container and release hardening is implemented for the current Phase One posture

Current status: publishable images are scanned in CI, built with SBOM and provenance attestations, signed with Cosign, and paired with explicit runtime hardening guidance for read-only deployments.

Evidence:

- `Dockerfile#L43-L75`
- `.github/workflows/build-images.yml#L63-L90`
- `justfile#L205-L231`

Remediation checklist:

- [x] Promote image scanning from a local-only recipe into the CI image pipeline.
- [x] Decide and document the supply-chain posture for provenance, signing, and SBOM publication.
- [x] Add the chosen provenance and signing steps to release automation if they are required for published images.
- [x] Revisit runtime hardening requirements such as read-only root guidance, writable mount expectations, and dropped capabilities.
- [x] Update operator docs so the intended runtime contract is explicit.

Done when:

- [x] Image scanning runs automatically in CI for publishable images.
- [x] Provenance and signing status is explicit, implemented, and documented.
- [x] Runtime hardening expectations are enforced or intentionally documented as deployment guidance.

### [x] Roadmap and documentation drift no longer hides repo truth

Current status: the roadmap, README, and operator-facing status docs have been re-baselined against the code so they no longer report completed work as missing.

Evidence:

- `docs/phase-one-roadmap.md#L11-L18`
- `.github/workflows/ci.yml#L139-L150`
- `crates/revaer-api/src/http/auth.rs#L679-L701`

Remediation checklist:

- [x] Update `docs/phase-one-roadmap.md` to remove completed items from the gap list.
- [x] Re-baseline the “current state” sections against the codebase before adding new work items.
- [x] Align README, roadmap, and other operator-facing status docs so they describe the same implementation reality.
- [x] Add a checklist item to future task records requiring status-doc validation whenever roadmap or README status claims are touched.

Done when:

- [x] The roadmap no longer reports completed features as missing.
- [x] Status docs agree on the same current implementation state.
- [x] Future planning includes a task-record check for status-doc validation when status claims change.

## Additional Findings Discovered During Implementation

### [x] Minimal-feature builds exposed a compat dead-code leak

Finding: the qB compatibility work introduced metadata that was only used when `compat-qb` was enabled, which caused `--no-default-features` builds to fail the repo’s dead-code rules.

Resolution:

- [x] Gate the compat-only `display_name` metadata behind the `compat-qb` feature.
- [x] Add the minimal-feature API/app test pass to the verification loop and rerun it after the fix.

Evidence:

- `crates/revaer-api/src/http/handlers/torrents/mod.rs`
- `justfile`

### [x] UI E2E auth overlay races came from mismatched session seeding

Finding: the UI suite sometimes started in anonymous mode while the paired API project had already established an authenticated server state, which left the auth overlay fighting real navigation.

Resolution:

- [x] Persist the API project auth session into shared E2E state.
- [x] Seed the browser fixture from that shared state before UI navigation begins.
- [x] Re-run the full Playwright suite until the overlay race is gone.

Evidence:

- `tests/fixtures/api.ts`
- `tests/fixtures/app.ts`
- `tests/pages/app-shell.ts`
- `tests/support/e2e-state.ts`

### [x] UI shell bootstrap was oversubscribed at the default Playwright worker count

Finding: the shell routes themselves were healthy, but the previous default of six UI workers caused the first route in each browser worker to contend hard enough that the shell bootstrap timed out intermittently.

Resolution:

- [x] Confirm the failing shell routes pass when isolated from the high-concurrency startup wave.
- [x] Reduce the default `ui-chromium` worker count to a safer baseline while keeping `E2E_UI_WORKERS` as an override for faster hosts.
- [x] Re-run the full `just ui-e2e` suite until the shell-route timeouts are gone.

Evidence:

- `tests/playwright.config.ts`
- `tests/specs/ui/dashboard.spec.ts`
- `tests/specs/ui/health.spec.ts`
- `tests/specs/ui/navigation.spec.ts`

## Execution Order

- [x] Fix roadmap and status-doc drift first so implementation planning reflects current repo truth.
- [x] Land runtime and API gaps with tests, starting with dashboard, FsOps, qB compatibility, and OTEL wiring.
- [x] Harden operational automation and image/release workflows after product/runtime gaps are defined.
- [x] Re-run verification, update this document’s checkboxes, and remove any evidence that is no longer accurate.

## Verification

- [x] `just fmt`
- [x] `just lint`
- [x] `just udeps`
- [x] `just audit`
- [x] `just deny`
- [x] `just test`
- [x] `just cov`
- [x] `just build-release`
- [x] `just ui-e2e`
- [x] `just ci`
- [x] Re-read `REMEDIATION_PLAN.md` after implementation work to ensure completed findings are marked, stale evidence is removed, and the document still matches the codebase.
