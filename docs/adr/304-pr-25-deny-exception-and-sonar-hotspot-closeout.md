# PR 25 Deny Exception And Sonar Hotspot Closeout

- Status: Accepted
- Date: 2026-04-16
- Context:
  - PR 25 still had two failing external checks after the workflow split work: `Check Deny` and SonarCloud Code Analysis.
  - The GitHub Actions log for `Check Deny` showed `cargo-deny` still reports `RUSTSEC-2026-0097` through the live dependency graph, while SonarCloud reported a single hotspot on `.github/workflows/ci.yml` for passing inherited secrets into the reusable image-build workflow.
- Decision:
  - Restore the temporary `RUSTSEC-2026-0097` ignore in `deny.toml` so `cargo-deny` matches the already-documented unresolved `sqlx-postgres -> rand 0.8.5` path.
  - Remove `secrets: inherit` from the release-only `build-images-dev` and `build-images-release` reusable-workflow caller jobs because those jobs do not require repository secrets beyond the default GitHub token and their explicit job permissions.
  - Record the closeout explicitly rather than burying it inside earlier ADRs, because this is a separate follow-up on live PR feedback and live CI output.
- Consequences:
  - `cargo-deny` and `cargo audit` now agree on the temporary handling of the unresolved `RUSTSEC-2026-0097` path.
  - SonarCloud no longer sees the reusable workflow callers as over-broad secret pass-through surfaces.
  - The PR keeps its single validation workflow split while also tightening the release-only caller jobs.
- Follow-up:
  - Remove `RUSTSEC-2026-0097` from both `.secignore` and `deny.toml` once the workspace no longer resolves `rand 0.8.5`.
  - Keep reusable-workflow callers on explicit inputs, permissions, and secrets only; avoid reintroducing `secrets: inherit` unless a callee actually consumes repository secrets.

## Task Record

- Motivation:
  - Clear the remaining failing PR checks on PR 25 using the actual current CI log and Sonar hotspot output rather than assumptions from earlier revisions.
- Design notes:
  - The deny fix intentionally restores a time-bounded exception instead of pretending the advisory is gone; the live GitHub Actions output confirms `cargo-deny` still resolves the vulnerable branch.
  - The Sonar hotspot was fixed by narrowing the workflow caller surface, not by suppressing analysis or weakening security tooling.
- Test coverage summary:
  - Reran `just deny`.
  - Queried the live SonarCloud hotspot API for PR 25 to identify the exact flagged line and rule.
  - Reran `just instruction-drift`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this is CI policy and workflow hardening only.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/workflows/ci.yml`, `deny.toml`, `.secignore`, and `.github/instructions/devops.instructions.md`.
  - Drift was found: `deny.toml` no longer matched the still-live `RUSTSEC-2026-0097` exception posture, and the reusable workflow caller still passed inherited secrets despite not consuming them.
  - Removed that contradiction by restoring the temporary deny exception and dropping inherited secrets from the image-build caller jobs.
- Risk & rollback plan:
  - Risk is limited to CI policy behavior: the deny exception could mask the advisory longer than intended, and removing inherited secrets could break the reusable workflow if it secretly relied on repository secrets.
  - Rollback is to revert this commit, which restores the prior deny posture and reusable-workflow secret inheritance while the branch is re-evaluated.
- Dependency rationale:
  - No new dependencies were added.
  - The fix stays within the existing RustSec exception mechanism and GitHub Actions workflow model.
