# Release Tag Image Job Dependency Split

- Status: Accepted
- Date: 2026-04-16
- Context:
  - PR 25 split pull-request validation into `pr.yml` and kept post-merge and tag release work in `ci.yml`.
  - The remaining `build-images` job still declared `needs: [load-matrix, release-dev]`, even though `release-dev` only runs on `main`, which meant stable tag pushes could skip image publication before the tag branch of the job condition was evaluated.
- Decision:
  - Split image publication in `ci.yml` into `build-images-dev` for `main` pushes and `build-images-release` for stable tags.
  - Keep the shared reusable workflow and matrix source, but give the dev and release jobs separate prerequisites and tags.
  - Update the devops instruction file to record that tag image publication must not depend on `main`-only jobs.
- Consequences:
  - Stable tags can publish release images without inheriting a skipped `release-dev` dependency.
  - `main` dev image publication still waits for the dev release metadata it needs.
  - The release-only workflow remains single-purpose without reintroducing duplicate PR validation.
- Follow-up:
  - Recheck GitHub Actions on PR 25 to confirm the duplicate-check concern is resolved and that tag image publication remains reachable.
  - Keep future release-only workflow edits explicit about branch-specific prerequisites.

## Task Record

- Motivation:
  - Address the remaining PR review feedback on `ci.yml` and remove a real tag-release image-publication skip path.
- Design notes:
  - The fix preserves the existing reusable `build-images.yml` flow and only separates the caller jobs by branch-specific dependency needs.
  - The change intentionally avoids reintroducing PR validation into `ci.yml`; `pr.yml` stays the sole validation workflow.
- Test coverage summary:
  - Reran `just instruction-drift`.
  - Reran `just ui-e2e`.
  - Reran `just ci`.
- Observability updates:
  - No runtime observability surfaces changed; this is release workflow orchestration only.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/workflows/ci.yml`, `.github/instructions/devops.instructions.md`, and the open PR feedback on PR 25.
  - Drift was found: the release-only workflow contract was documented, but `ci.yml` still allowed a tag release path to depend on the `main`-only `release-dev` job.
  - Removed that contradiction by splitting dev and stable image publication and documenting the branch-specific dependency rule in the devops instruction file.
- Risk & rollback plan:
  - Risk is limited to release image publication paths if one of the new caller jobs has the wrong branch condition or reusable-workflow inputs.
  - Rollback is a revert of the job split if GitHub Actions exposes a regression in tag or `main` image publication.
- Dependency rationale:
  - No new dependencies were added.
  - The existing reusable image-build workflow was retained instead of introducing more workflow layers for a single dependency fix.
