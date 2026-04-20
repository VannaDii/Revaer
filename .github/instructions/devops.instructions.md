---
applyTo:
  - ".github/workflows/**"
  - ".github/actions/**"
  - "Dockerfile"
  - "release/**"
  - "sonar-project.properties"
---

`AGENTS.md` is the root contract. This file specializes workflows, release automation, container build files, and Sonar config.

# Workflow And Release Rules

- Use minimal GitHub token permissions at the workflow or job level. Only grant elevated scopes to the job that needs them.
- External GitHub actions in modified files must pin the exact upstream commit SHA. Do not use floating branch refs such as `main`, `master`, or `trunk`, and do not rely on mutable release tags alone.
- When updating an external action reference, resolve the chosen stable upstream release tag to its full 40-character commit SHA at the time of the change. Keep the originating tag in an inline comment when practical so upgrades stay auditable.
- Verify action usage against the action's current official documentation when changing its major or minor release line. Preserve documented step ordering and supported inputs.
- ORAS setup jobs in workflows must stay on a node24-capable `oras-project/setup-oras` release line and request an ORAS CLI version that the pinned action release explicitly supports.
- ORAS publish commands in release scripts must avoid absolute on-disk layer paths unless path validation is intentionally disabled; prefer running from the asset directory and pushing relative artifact names.
- Helm OCI publication defaults must target the owner-qualified GHCR namespace derived from the active GitHub repository. If a non-GitHub registry layout is needed, override it explicitly with `HELM_REGISTRY_NAMESPACE` rather than relying on an incomplete default path.
- Workflows that install Rust toolchains must use the repository's configured toolchain source of truth rather than hard-coded ad hoc channels unless a documented exception is required.
- Workflow build, lint, test, coverage, and release gates must call `just` recipes. Do not reintroduce raw `cargo` pipelines into CI jobs.
- `pr.yml` is the sole pull-request validation workflow. Keep formatting, lint, test, audit, deny, coverage, E2E, and other verification gates there so pull requests are validated exactly once before merge.
- `ci.yml` is the post-merge and tag-release workflow. Limit it to release-artifact, publish, and image-build activity for `main` pushes and release tags; do not duplicate PR validation jobs there.
- Manual release verification belongs in dedicated `workflow_dispatch` workflows, not in `pr.yml`, and should reuse the same `just` entrypoints and pinned third-party actions as the release path they exercise.
- Manual workflows that publish PR-scoped dev Helm artifacts should encode the PR number into the default prerelease version so registry output is traceable back to the reviewed change.
- Reusable image workflows may publish PR-scoped dev Helm charts only as an optional post-manifest job. Keep that publish step downstream of the multi-arch manifest job, drive it through `just helm-package` and `just helm-publish`, and derive the default prerelease chart version from the caller-provided PR number.
- Release-tag image publication in `ci.yml` must not depend on `release-dev` or any other `main`-only job. Split dev and tag image publishing into separate jobs when their prerequisites differ.
- Stable tag activity in `ci.yml` must exclude prerelease tags consistently at the job boundary, not only in downstream publish jobs. Do not let prerelease tags build stable release artifacts that the later jobs refuse to publish.
- Reusable-workflow caller jobs must not use `secrets: inherit` unless the callee truly requires repository secrets. Prefer the default GitHub token plus explicit job permissions, and pass named secrets only when the callee consumes them.
- Helm chart validation and publication must flow through `just helm-lint`, `just helm-package`, and `just helm-publish`. Do not add ad hoc packaging or registry-push shell blocks to workflows.
- Workflow jobs that invoke `just helm-lint` must install `just` first through `./.github/actions/setup-revaer`; do not assume the runner image already provides it.
- `just lint` runs `scripts/workflow-guardrails.sh`, which rejects unpinned external action refs and direct `${{ inputs.* }}` interpolation inside `run:` blocks.
- Treat `sonar-project.properties` as the versioned source of truth for Sonar analysis scope and exclusions.
- Release-tooling dependency changes under `release/**`, including JavaScript lockfiles such as `release/package-lock.json`, must stay manifest-scoped, avoid unrelated workflow churn, and update this instruction file in the same change so instruction-drift remains explicit.
- Prerelease Helm assets must be produced during the semantic-release prepare phase so the packaged chart version matches the dev release version exactly. OCI publication must consume those already-packaged assets after the GitHub release assets exist.
- Stable tag releases must package the Helm chart once, attach the `.tgz`, `.prov`, and public key to the GitHub release, and publish that exact packaged chart to the OCI registry. Avoid repackaging between release-asset upload and OCI publication.
- JavaScript release metadata helpers under `release/**` should stay side-effect scoped. Prefer wiring shell packaging steps in the semantic-release `prepareCmd` over spawning child processes from Node glue unless a documented exception is required.
- Semantic-release command templates under `release/**` must remain lodash-template-safe. Do not embed shell parameter-expansion forms such as `${VAR:-default}` inside configured command strings because lodash templating parses the same `${...}` syntax first. Semantic-release placeholders such as `${nextRelease.version}` remain allowed; for shell conditionals prefer plain shell variable references such as `"$VAR"` or move the conditional logic into a script.
- Helm packaging scripts must exclude repository-level Artifact Hub metadata from the chart tarball itself. Publish `artifacthub-repo.yml` as a separate OCI artifact instead of shipping it inside the chart package.
- Helm publishing must verify signed chart artifacts before OCI push, and temporary exported secret keyring files must be created with owner-only permissions.

# Shell Safety

- Never interpolate untrusted `${{ inputs.* }}` or comparable expression values directly into `run:` blocks.
- Map user-controlled inputs into environment variables first, validate or whitelist them, then consume them in shell.
- Prefer arrays and quoted expansions over word-splitting command strings.
- Setup-action package-list inputs may accept general shell whitespace, including CRLF-pasted multiline input, when that improves YAML readability, but the resulting tokens must still be normalized into a validated array before invocation.

# Credentials And Test Infrastructure

- CI-only credentials may be ephemeral only when they are clearly scoped to isolated test infrastructure, such as throwaway Postgres service containers.
- Ephemeral test credentials must never be reused as application secrets, committed runtime credentials, or user-facing examples.
- Do not log secrets or secret-like values. Mask or omit them.
- Keep Helm registry credentials (`HELM_API_KEY_ID`, `HELM_API_KEY_SECRET`) separate from chart-signing material (`HELM_GPG_PRIVATE`, `HELM_GPG_PUBLIC`). Publishing jobs may use registry credentials only when consuming an already-packaged chart artifact.

# Drift Control

- Any change to a workflow, release script, setup action, `justfile`, or `sonar-project.properties` must review the matching instruction file in the same change.
- Revaer enforces that rule mechanically with `just instruction-drift`, backed by `scripts/instruction-drift-check.sh`. Keep the mapping in that script aligned with this file and `AGENTS.md`.
- Keep `scripts/workflow-guardrails.sh` aligned with the live workflow policy when GitHub Actions pinning or shell-safety rules change.
- `pr.yml` must pass explicit base/head SHAs into `just instruction-drift` so pull requests are checked against the real reviewed diff, not an incidental worktree state.
- Drift coverage for actions and release assets is recursive. Changes under `.github/actions/**`, `.github/workflows/**`, and `release/**` must keep matching the devops instruction update rule.
- Reusable workflows that publish images must preserve `packages: write` on the caller job because the callee cannot elevate a more restrictive token.
- Reusable-workflow caller jobs must define one merged `permissions` map. Do not duplicate the `permissions` key in a job to append scopes later; GitHub Actions rejects the workflow before execution.
- Keep the Sonar PR gate blocking and decoration-based. Do not add `sonar.qualitygate.wait=true` to PR scans unless the branch-protection model cannot consume Sonar’s status directly.
