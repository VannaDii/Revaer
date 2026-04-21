# PR Helm Review Follow-Ups

- Status: Accepted
- Date: 2026-04-19
- Context:
  - PR review on the PR-scoped Helm publish work flagged workflow shell-safety gaps, overly broad workflow permissions, reusable-workflow secret inheritance, and a filename drift hazard in Helm metadata publication.
  - The repository policy requires workflow and release-script changes to stay aligned with the devops instruction set and task-record ADR bookkeeping.
- Decision:
  - Harden `helm-oci-verify.yml` by validating manual version inputs, writing outputs with the multiline `GITHUB_OUTPUT` form, and moving step-consumed values through `env`.
  - Narrow workflow permissions to the jobs that need them, guard the PR Sonar scan for non-fork PRs with configured tokens, and replace reusable-workflow `secrets: inherit` with explicit Helm publishing secrets.
  - Make `release/scripts/helm-publish.sh` push Artifact Hub metadata by the derived metadata filename so the script stays correct if the metadata path changes.
- Consequences:
  - The PR and manual Helm workflows are tighter against shell injection, privilege creep, and secret overexposure.
  - Manual verification inputs are stricter; unsupported chart or app version formats now fail fast instead of reaching downstream tooling.
- Follow-up:
  - Keep future workflow-dispatch publish inputs on the same validate-then-export pattern.
  - Preserve the explicit reusable-workflow secret contract if Helm publish steps move again.

## Task Record

- Motivation:
  - Close the open PR review threads on the Helm publish work without widening workflow scope beyond the reviewed areas.
- Design notes:
  - `chart_version` now uses a SemVer-compatible validation regex because Helm chart versions must stay SemVer-shaped.
  - `app_version` stays intentionally narrower than arbitrary shell text because it is only used as a release identifier, not as a free-form note field.
  - `pull-requests: read` moved from workflow scope to the `coverage` and manual Helm verification jobs that actually need it.
  - The reusable image workflow call now receives only the four Helm secrets it consumes.
- Test coverage summary:
  - `just instruction-drift`
  - `just ci`
  - `just ui-e2e`
- Observability updates:
  - No runtime observability surface changed.
  - Workflow failures now report invalid manual version inputs at the validation step before packaging or publishing.
- Status-doc validation:
  - Reviewed `.github/instructions/devops.instructions.md`, `docs/adr/index.md`, and `docs/SUMMARY.md`; updated them to match the new workflow and release-script constraints.
- Risk & rollback plan:
  - Main risk is rejecting a previously tolerated manual version override. Roll back by reverting this ADR and the corresponding workflow/script changes if a legitimate version format was excluded.
  - Permission and secret changes are isolated to PR/manual workflow paths and can be reverted with a single commit if a reusable workflow contract was missed.
- Dependency rationale:
  - No new dependencies were added.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - Drift found: the instruction set did not yet capture validated `workflow_dispatch` inputs or safe multiline `$GITHUB_OUTPUT` writes for workflow shell surfaces.
  - Removed that drift by updating the devops instructions in this change.
