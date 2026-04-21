# GHCR Helm GitHub Token Authentication

- Status: Accepted
- Date: 2026-04-20
- Context:
  - PR run `24643631011` failed in `Images / Helm Chart` after the chart package and signature verification completed.
  - The shared `release/scripts/helm-publish.sh` path reached `helm registry login ghcr.io` and GHCR returned `403 denied` when the workflow used the configured Helm API secret pair.
  - The same shared publish path is reused by the PR reusable workflow and the `main` and tag publish jobs in `ci.yml`.
- Decision:
  - Teach `release/scripts/helm-publish.sh` to accept explicit `HELM_REGISTRY_USERNAME` and `HELM_REGISTRY_PASSWORD`, with a `GITHUB_TOKEN` fallback for `ghcr.io`.
  - Update GitHub-hosted GHCR publish jobs to pass `github.actor` plus `secrets.GITHUB_TOKEN` instead of the long-lived Helm API secret pair.
  - Grant `packages: write` to the `ci.yml` Helm publish jobs so the repo token can publish to GHCR.
- Consequences:
  - PR, `main`, and tag Helm publication paths now authenticate to GHCR with the job-scoped repository token.
  - Local or non-GitHub publish rehearsals can still use `HELM_API_KEY_*` or the new explicit registry credential variables.
- Follow-up:
  - Re-run the PR `Images / Helm Chart` job and confirm GHCR login and chart push succeed.
  - Keep non-GitHub registry callers on explicit override credentials instead of assuming GHCR defaults.

## Task Record

- Motivation:
  - Restore the failing PR Helm chart publish job and align the shared Helm publish path with GitHub-hosted GHCR auth.
- Design notes:
  - The credential selection now prefers explicit `HELM_REGISTRY_*` values, then existing `HELM_API_KEY_*`, then `GITHUB_TOKEN` for `ghcr.io`.
  - The reusable PR workflow already had `packages: write`; the `ci.yml` Helm publish jobs needed that permission added to use `GITHUB_TOKEN`.
- Test coverage summary:
  - Inspected GitHub Actions run `24643631011`, job `72055391065`, and confirmed the failure occurred during GHCR authentication after successful packaging and signature verification.
  - `just ci`
  - `just ui-e2e`
- Observability updates:
  - No runtime observability surface changed.
  - Publish failures now report the accepted credential sources more clearly from `helm-publish.sh`.
- Status-doc validation:
  - Reviewed `.github/instructions/devops.instructions.md`, `docs/adr/index.md`, and `docs/SUMMARY.md`; updated them to match the GHCR auth path.
- Risk & rollback plan:
  - Main risk is a missing `packages: write` permission on a future caller job. Roll back by reverting this change or restoring explicit non-GitHub credentials for that caller.
  - Local and non-GitHub publish flows can still pin explicit credentials if the GitHub-token path is unsuitable.
- Dependency rationale:
  - No new dependencies were added.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - Drift found: the instruction set did not yet record that GitHub-hosted GHCR chart publication should prefer the job-scoped repo token.
  - Removed that drift by updating the devops instructions in this change.
