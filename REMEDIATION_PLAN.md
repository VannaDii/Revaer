# Revaer Remediation Plan

This checklist is based on current code and documentation inspection as of 2026-04-04. It is intended to track verified implementation and process gaps to closure without copying stale roadmap items forward uncritically.

## Current Findings

### [ ] Dashboard API is still stubbed

Current gap: `/v1/dashboard` returns hard-coded zero values instead of live engine, filesystem, or telemetry-backed data.

Evidence:

- `crates/revaer-api/src/http/handlers/health.rs#L89-L107`
- `docs/phase-one-roadmap.md#L14-L17`

Remediation checklist:

- [ ] Define a dashboard snapshot source of truth owned by runtime state rather than the handler.
- [ ] Source transfer rates, torrent counts, and disk usage from live collaborators already injected into the API layer.
- [ ] Replace the hard-coded zero payload in the dashboard handler with the live snapshot.
- [ ] Define explicit fallback behavior when backing state is unavailable and return safe values without silently masking failure causes.
- [ ] Add API tests that prove non-stub behavior when runtime state is populated.
- [ ] Add API tests that prove the fallback path when snapshot dependencies are unavailable or degraded.

Done when:

- [ ] `/v1/dashboard` reflects live values from runtime state.
- [ ] The handler no longer contains hard-coded metric placeholders.
- [ ] Tests cover both healthy and degraded snapshot sourcing behavior.

### [ ] FsOps archive, PAR2, and checksum support is incomplete

Current gap: archive extraction is limited to `zip`, PAR2 policy exists in config but is not implemented in the runtime pipeline, and checksum-aware cleanup metadata is absent.

Evidence:

- `crates/revaer-fsops/src/service/mod.rs#L910-L927`
- `crates/revaer-config/src/model.rs#L541-L555`
- `docs/phase-one-roadmap.md#L13-L13`

Remediation checklist:

- [ ] Expand archive handling beyond `zip` to the supported format set agreed for Phase One.
- [ ] Decide whether archive tooling is library-backed, command-backed, or mixed, and document the rationale in the implementation task record.
- [ ] Implement PAR2 verify and repair stages that honor the configured `FsPolicy.par2` mode.
- [ ] Record checksum metadata alongside `.revaer.meta` when cleanup or validation behavior depends on file identity.
- [ ] Document and implement non-Unix fallback behavior for chmod/chown/umask handling if portability remains a requirement.
- [ ] Add FsOps tests that cover supported archive formats, PAR2 decisions, checksum persistence, and fallback behavior.

Done when:

- [ ] The pipeline supports the intended archive matrix instead of failing for every non-zip archive.
- [ ] PAR2 policy produces observable runtime behavior rather than config-only state.
- [ ] Checksum handling is either implemented with tests or explicitly removed from the roadmap/spec if not required.

### [ ] qBittorrent compatibility is not feature-complete

Current gap: the compatibility router exposes a useful subset of qBittorrent endpoints, but common mutation routes are still missing.

Evidence:

- `crates/revaer-api/src/http/handlers/compat_qb.rs#L41-L66`
- `docs/phase-one-roadmap.md#L14-L14`

Remediation checklist:

- [ ] Define the intended qBittorrent parity scope so compatibility has a bounded, testable target.
- [ ] Add the missing high-value mutation endpoints, including rename, relocate, category/tag mutation, recheck, and reannounce if those remain in scope.
- [ ] Reuse existing domain services rather than duplicating mutation logic inside the compatibility layer.
- [ ] Add compatibility tests for each supported route, including request shape, auth behavior, and response semantics.
- [ ] Update the roadmap and API docs to describe exactly which qB endpoints are supported and which are intentionally out of scope.

Done when:

- [ ] Supported qB endpoints are explicitly documented.
- [ ] The highest-priority missing mutation routes are implemented and tested.
- [ ] Remaining omissions are deliberate and documented rather than accidental.

### [ ] OpenTelemetry export is still placeholder-level

Current gap: the repo advertises optional OTEL support, but the current implementation only builds a local tracing layer and keeps the exporter endpoint as a placeholder for future wiring.

Evidence:

- `README.md#L77-L79`
- `crates/revaer-telemetry/src/init.rs#L122-L130`
- `crates/revaer-telemetry/src/init.rs#L198-L207`

Remediation checklist:

- [ ] Replace placeholder OTEL wiring with a real exporter path that is enabled only when OTEL configuration is explicitly turned on.
- [ ] Preserve the current dormant-by-default behavior when OTEL is disabled.
- [ ] Validate configuration early and surface actionable startup errors for invalid exporter settings.
- [ ] Add smoke tests or focused unit tests for OTEL disabled and OTEL enabled initialization flows.
- [ ] Update README and operator docs to describe the actual supported OTEL configuration surface.

Done when:

- [ ] OTEL configuration drives a real exporter path rather than storing an unused endpoint.
- [ ] Disabled OTEL remains a no-op.
- [ ] Tests cover enabled and disabled initialization behavior.

### [ ] Operational validation is still manual

Current gap: the runbook exists as a human checklist, but the operational validation gate is not automated through a repeatable `just`-driven flow.

Evidence:

- `docs/runbook.md#L1-L53`
- `scripts/dev-seed.sql`
- `docs/phase-one-roadmap.md#L18-L18`

Remediation checklist:

- [ ] Convert the runbook’s critical validation steps into one or more `just` recipes that can be executed locally and in CI where feasible.
- [ ] Keep `docs/runbook.md` as the human-readable operator guide, but make it reference the automation commands instead of duplicating manual steps blindly.
- [ ] Define the expected artifacts for successful operational validation, including health snapshots, metrics captures, and event traces where appropriate.
- [ ] Add clear prerequisites and failure output so the automated runbook is actionable when it breaks.
- [ ] Decide which scenarios remain manual-only and document why.

Done when:

- [ ] The key runbook scenarios are executable through `just` recipes or another checked-in automation entrypoint.
- [ ] Manual steps are limited to cases that truly cannot be automated.
- [ ] The runbook points to a repeatable validation flow instead of serving as the only execution path.

### [ ] Container and release hardening is incomplete

Current gap: image builds are functional and non-root, but image scanning is not enforced in the image workflow, provenance/signing is absent, and runtime hardening expectations are not fully implemented or documented.

Evidence:

- `Dockerfile#L43-L75`
- `.github/workflows/build-images.yml#L63-L90`
- `justfile#L205-L231`

Remediation checklist:

- [ ] Promote image scanning from a local-only recipe into the CI image pipeline.
- [ ] Decide and document the supply-chain posture for provenance, signing, and SBOM publication.
- [ ] Add the chosen provenance and signing steps to release automation if they are required for published images.
- [ ] Revisit runtime hardening requirements such as read-only root guidance, writable mount expectations, and dropped capabilities.
- [ ] Update operator docs so the intended runtime contract is explicit.

Done when:

- [ ] Image scanning runs automatically in CI for publishable images.
- [ ] Provenance and signing status is explicit, implemented, and documented.
- [ ] Runtime hardening expectations are enforced or intentionally documented as deferred work.

### [ ] Roadmap and documentation drift is hiding repo truth

Current gap: the roadmap still lists several gaps that are already implemented, which makes it harder to prioritize the real work.

Evidence:

- `docs/phase-one-roadmap.md#L11-L18`
- `.github/workflows/ci.yml#L139-L150`
- `crates/revaer-api/src/http/auth.rs#L679-L701`

Remediation checklist:

- [ ] Update `docs/phase-one-roadmap.md` to remove completed items from the gap list.
- [ ] Re-baseline the “current state” sections against the codebase before adding new work items.
- [ ] Align README, roadmap, and other operator-facing status docs so they describe the same implementation reality.
- [ ] Add a checklist item to future task records requiring status-doc validation whenever roadmap or README status claims are touched.

Done when:

- [ ] The roadmap no longer reports completed features as missing.
- [ ] Status docs agree on the same current implementation state.
- [ ] Future planning can rely on the docs without an extra verification sweep.

## Execution Order

- [ ] Fix roadmap and status-doc drift first so implementation planning reflects current repo truth.
- [ ] Land runtime and API gaps with tests, starting with dashboard, FsOps, qB compatibility, and OTEL wiring.
- [ ] Harden operational automation and image/release workflows after product/runtime gaps are defined.
- [ ] Re-run verification, update this document’s checkboxes, and remove any evidence that is no longer accurate.

## Verification

- [ ] `just fmt`
- [ ] `just lint`
- [ ] `just udeps`
- [ ] `just audit`
- [ ] `just deny`
- [ ] `just test`
- [ ] `just cov`
- [ ] `just build-release`
- [ ] `just ui-e2e`
- [ ] Re-read `REMEDIATION_PLAN.md` after implementation work to ensure completed findings are marked, stale evidence is removed, and the document still matches the codebase.
