# PR 25 Prerelease Tag Release Guard

- Status: Accepted
- Date: 2026-04-16
- Context:
  - After splitting PR validation and release-only workflow responsibilities, `ci.yml` still allowed `build-release` to run on prerelease tags because the workflow trigger matched `v*.*.*` and the stable-tag filter only existed on downstream publish jobs.
  - PR 25 had an unresolved review thread calling out that prerelease tags such as `v1.2.3-rc.1` could still build and upload stable release artifacts even though later publish jobs correctly skipped them.
- Decision:
  - Add a job-level guard on `build-release` so prerelease tags are excluded at the point stable release artifacts would otherwise be created.
  - Update the devops instruction file to require stable-tag exclusion at the job boundary, not only in downstream publish steps.
- Consequences:
  - Stable release artifact creation now matches the stable-tag-only contract already used by the later publish jobs.
  - Prerelease tags no longer produce misleading stable release artifacts in `ci.yml`.
  - The PR thread can be resolved with an actual workflow fix rather than an explanation-only response.
- Follow-up:
  - Keep future release-only tag jobs aligned on the same prerelease exclusion rule.
  - If prerelease artifact publication is needed later, add an explicit prerelease path instead of letting the stable release path partially run.

## Task Record

- Motivation:
  - Close the remaining actionable PR feedback item on release-tag behavior with a minimal workflow fix.
- Design notes:
  - The change is intentionally narrow: it preserves the existing trigger surface and downstream stable-release guards, and adds the missing stable-tag filter to the release-artifact job itself.
- Test coverage summary:
  - Reran `just instruction-drift`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this work only tightens release workflow orchestration.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/workflows/ci.yml`, and `.github/instructions/devops.instructions.md`.
  - Drift was found: the documented stable-release-only tag intent was not enforced uniformly because `build-release` still ran on prerelease tags.
  - Removed that contradiction by adding the prerelease tag guard to `build-release` and documenting the rule in the devops instruction file.
- Risk & rollback plan:
  - Risk is limited to release automation; an overly broad guard could skip legitimate stable release builds.
  - Rollback is a revert of this commit if stable tags stop producing release artifacts unexpectedly.
- Dependency rationale:
  - No new dependencies were added.
  - The fix stays within the existing workflow and policy files rather than introducing new release automation layers.
