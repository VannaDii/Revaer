# PR Validation And Main Release Workflow Split

- Status: Accepted
- Date: 2026-04-16
- Context:
  - Both `.github/workflows/pr.yml` and `.github/workflows/ci.yml` were running the same validation graph on pull requests, which duplicated formatting, lint, test, coverage, audit, deny, and E2E work.
  - The repository cannot merge directly to `main`, so pull requests are the enforced validation boundary before any post-merge or tag release activity happens.
- Decision:
  - Keep all pull-request validation in `.github/workflows/pr.yml`.
  - Restrict `.github/workflows/ci.yml` to release-only work for `main` pushes and stable tags: building release artifacts, publishing releases, publishing Helm charts, and building images.
  - Update the devops instruction file to make the PR-validation-versus-release-workflow split explicit.
- Consequences:
  - Pull requests no longer pay for two copies of the same validation graph.
  - `main` pushes and stable tags keep the release pipeline they need without reopening the full validation matrix after merge.
  - Future workflow edits have a clearer contract for where verification belongs and where release automation belongs.
- Follow-up:
  - Monitor PR and `main` workflow runtimes after the split to confirm the duplicate validation load is gone.
  - If more release-only steps are added later, keep them in `ci.yml` unless they are required to validate a pull request before merge.

## Task Record

- Motivation:
  - Remove duplicated PR validation work and align workflow ownership with the repository's branch-protection model.
- Design notes:
  - `.github/workflows/ci.yml` now triggers only on `push` to `main` and release tags and contains release-artifact, publish, Helm, and image-build jobs only.
  - `.github/workflows/pr.yml` remains the only workflow that runs instruction drift, lint, tests, audit, deny, coverage, and UI E2E checks for pull requests.
- Test coverage summary:
  - Reran `just instruction-drift`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this work only changes GitHub Actions workflow boundaries.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/workflows/ci.yml`, `.github/workflows/pr.yml`, and `.github/instructions/devops.instructions.md`.
  - Drift was found: the workflow pair still duplicated PR validation despite the repository relying on PRs as the enforced validation boundary.
  - Removed the stale overlap by making `pr.yml` the sole validation workflow and updating the devops instruction text to document that split.
- Risk & rollback plan:
  - Risk is missing a validation guard after merge if a needed check was accidentally removed from both workflows.
  - Rollback is to revert the workflow split commit, which restores the old duplicated validation behavior immediately.
- Dependency rationale:
  - No new dependencies were added.
  - The change reuses the existing workflows and reusable image-build flow rather than introducing a new reusable validation workflow in the same change.
