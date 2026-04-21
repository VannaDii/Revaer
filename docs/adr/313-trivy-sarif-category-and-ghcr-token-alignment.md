# Trivy SARIF Category And GHCR Token Alignment

- Status: Accepted
- Date: 2026-04-20
- Context:
  - PR #27 moved image builds into a reusable workflow and added a manual Helm OCI verification workflow.
  - GitHub Advanced Security started reporting `2 configurations not found` for the Trivy scan because the workflow/job identity that code scanning used on `main` (`.github/workflows/ci.yml:build-images/...`) no longer matched the PR branch upload identity after the refactor.
  - Review feedback also flagged that the manual GHCR publish path still preferred legacy Helm API secrets instead of the job-scoped `GITHUB_TOKEN`, and that the reusable image workflow still trusted the `pr_number` input too much when writing environment values.
- Decision:
  - Set an explicit SARIF upload category in the reusable image workflow that preserves the legacy `ci.yml:build-images` matrix identity for Trivy uploads.
  - Validate `pr_number` as numeric and use the multiline `$GITHUB_ENV` form before exporting PR-scoped Helm version values.
  - Switch the manual Helm OCI verification workflow to GHCR publication through `GITHUB_TOKEN` plus `packages: write`.
  - Remove unused `HELM_API_KEY_*` secret plumbing from the reusable PR image workflow call and drop stale `pull-requests: read` permission from the push-only Sonar workflow.
- Consequences:
  - GitHub code scanning can compare PR Trivy uploads to the existing `main` configurations instead of treating them as missing configurations after the workflow refactor.
  - Manual GHCR verification now exercises the same credential path used by the GitHub-hosted publish jobs.
  - Reusable workflow callers expose fewer secrets and PR-number-derived env writes are hardened against newline or non-numeric injection.
- Follow-up:
  - Re-run PR #27 checks and confirm the Trivy configuration warning disappears.
  - Confirm the manual Helm OCI verification workflow can publish with `GITHUB_TOKEN` on a GitHub-hosted runner.

## Task Record

- Motivation:
  - Restore trustworthy PR code-scanning comparisons and close the remaining workflow review threads on PR #27 without regressing least-privilege rules.
- Design notes:
  - The SARIF category is intentionally pinned to the historical `ci.yml` build-image key instead of the reusable workflow path because code scanning continuity matters more than reflecting the refactor in the category string.
  - The manual Helm verify workflow keeps `pull-requests: read` because it still resolves an open PR number from the branch when inputs are omitted.
- Test coverage summary:
  - `just instruction-drift`
  - `just ci`
  - `just ui-e2e`
- Observability updates:
  - No runtime observability surface changed.
  - GitHub code-scanning continuity for Trivy uploads should recover once the workflow reruns.
- Status-doc validation:
  - Re-checked `.github/instructions/devops.instructions.md`, `docs/adr/index.md`, and `docs/SUMMARY.md`; updated them to match the workflow behavior.
- Risk & rollback plan:
  - The main risk is pinning the SARIF category to the legacy identity longer than desired. Roll back by changing the explicit category once the old code-scanning configurations are intentionally retired.
  - If `GITHUB_TOKEN` proves insufficient for the manual GHCR publish path, restore explicit registry credentials as a documented exception.
- Dependency rationale:
  - No new dependencies were added.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - Drift found: the instructions did not yet record the need to preserve a stable Trivy SARIF category across workflow refactors.
  - Removed that drift by updating the workflow and instruction file together.
