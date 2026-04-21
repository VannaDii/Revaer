# GHCR Helm Namespace Derivation

- Status: Accepted
- Date: 2026-04-19
- Context:
  - The PR-side `Publish Dev Helm Chart` job reached `just helm-publish` and then failed against GHCR with `response status code 403: denied: denied`.
  - `release/scripts/helm-publish.sh` defaulted the OCI namespace to `revaer/charts`, which omits the GitHub owner segment required by GHCR package scopes.
  - That incorrect default affected every workflow path that reuses `just helm-publish`, not only the PR-side reusable image workflow.
- Decision:
  - Derive the default Helm OCI namespace from `GITHUB_REPOSITORY` when available, lowercased and suffixed with `/charts`.
  - Keep `HELM_REGISTRY_NAMESPACE` as an explicit override so local disposable registry tests and any future non-GitHub targets can still set a custom namespace.
  - Update the operator-facing release checklist to describe the owner/repo-qualified GHCR path.
- Consequences:
  - PR, `main`, and manual Helm publish flows now target the same owner-qualified GHCR namespace by default.
  - Existing callers that already provide `HELM_REGISTRY_NAMESPACE` keep their current behavior.
  - Release documentation now matches the actual GHCR package location instead of the incomplete legacy path.
- Follow-up:
  - Re-run the failing PR publish path and confirm GHCR authentication succeeds with the owner-qualified namespace.
  - Refresh any remaining docs or automation that still reference `ghcr.io/<owner>/<repo>/charts/...`.

## Task Record

- Motivation:
  - Restore the failing PR-side Helm publish job and avoid repeating the same GHCR namespace bug in the main and manual publish paths.
- Design notes:
  - The fix stays in `release/scripts/helm-publish.sh` so all workflow entrypoints that call `just helm-publish` inherit the correction automatically.
  - `GITHUB_REPOSITORY` is the most stable source because it already includes both owner and repo, and GHCR package paths are case-insensitive but normalized to lowercase.
- Test coverage summary:
  - Verified the failing GitHub Actions job log for run `24639626283`, job `72043750922`, and confirmed the GHCR 403 denial happened during `just helm-publish`.
  - Planned verification: `just ci`.
  - Planned verification: `just ui-e2e`.
  - Planned verification: rerun the PR-side `Publish Dev Helm Chart` job and confirm GHCR authentication and push succeed.
- Observability updates:
  - No runtime observability surfaces changed; this task only corrects CI/release publication configuration.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/instructions/devops.instructions.md`, `release/scripts/helm-publish.sh`, and `docs/release-checklist.md`.
  - Drift was found: Helm publication docs and defaults referenced an incomplete GHCR namespace that omitted the repository owner.
  - Removed that drift by deriving the namespace from `GITHUB_REPOSITORY` and updating the checklist path.
- Risk & rollback plan:
  - Risk is limited to chart publication paths. If a non-GitHub environment depends on the old default, it can still restore that behavior by setting `HELM_REGISTRY_NAMESPACE`.
  - Rollback is a revert of this script/doc change or an explicit workflow-level namespace override.
- Dependency rationale:
  - No repository dependencies were added.
  - The fix reuses existing GitHub-provided environment metadata instead of adding workflow glue or new tooling.
