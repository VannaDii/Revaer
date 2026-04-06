# PR 21 Trivy action pin refresh

- Status: Accepted
- Date: 2026-04-05
- Context:
  - PR 21 image-build jobs started failing during `Set up job` before any Docker or Trivy work executed.
  - The reusable image workflow pins `aquasecurity/trivy-action` and must stay stable for both PR image previews and release image builds.
- Decision:
  - Refresh the pinned `aquasecurity/trivy-action` revision in the reusable image workflow to the current `v0.35.0` commit.
  - Avoid adding a bespoke Trivy bootstrap workaround because the failure came from a broken upstream dependency reference in the older pinned action revision.
- Consequences:
  - PR and release image scans use a current upstream action revision that resolves its internal `setup-trivy` dependency correctly.
  - Future upstream breakage still requires periodic pin review, but the workflow returns to a working pinned state without changing scan policy.
- Follow-up:
  - Re-run the PR image workflow and confirm both architecture builds plus the multi-arch manifest job report status normally.
  - Keep the Trivy action pin aligned with upstream security maintenance when workflow dependencies are refreshed again.

## Task Record

- Motivation:
  - PR 21 was blocked by failing `Build PR Images` jobs, which in turn kept the required image workflow from completing.
- Design notes:
  - The fix stays inside `.github/workflows/build-images.yml` because the break was in the reusable image workflow's pinned third-party action revision.
  - The updated pin targets the upstream `v0.35.0` commit `57a97c7e7821a5776cebc9bb87c984fa69cba8f1`, whose composite action installs Trivy through a pinned `setup-trivy` commit instead of the missing `v0.2.1` tag that broke the older revision.
- Test coverage summary:
  - Re-ran `PG_VOLUME=revaer-pgdata-ci just ui-e2e`.
  - Re-ran `PG_VOLUME=revaer-pgdata-ci just ci`.
  - Pulled PR 21 workflow logs to confirm the old failure signature before applying the pin refresh.
- Observability updates:
  - No runtime observability surfaces changed; this is CI workflow maintenance only.
- Status-doc validation:
  - `README.md` and operator-facing docs were re-checked and do not describe the pinned Trivy action revision, so no user-facing doc update was required.
- Risk & rollback plan:
  - Risk is limited to CI image scanning behavior on PR and release workflows.
  - Rollback is a single-commit revert of the workflow pin if the newer Trivy action regresses unexpectedly.
- Dependency rationale:
  - No new dependencies were added.
  - Updating the existing pinned action was preferred over embedding custom Trivy installation logic or disabling image scanning.
