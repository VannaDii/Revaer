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
- Workflows that install Rust toolchains must use the repository's configured toolchain source of truth rather than hard-coded ad hoc channels unless a documented exception is required.
- Workflow build, lint, test, coverage, and release gates must call `just` recipes. Do not reintroduce raw `cargo` pipelines into CI jobs.
- `just lint` runs `scripts/workflow-guardrails.sh`, which rejects unpinned external action refs and direct `${{ inputs.* }}` interpolation inside `run:` blocks.
- Treat `sonar-project.properties` as the versioned source of truth for Sonar analysis scope and exclusions.

# Shell Safety

- Never interpolate untrusted `${{ inputs.* }}` or comparable expression values directly into `run:` blocks.
- Map user-controlled inputs into environment variables first, validate or whitelist them, then consume them in shell.
- Prefer arrays and quoted expansions over word-splitting command strings.
- Setup-action package-list inputs may accept general shell whitespace, including CRLF-pasted multiline input, when that improves YAML readability, but the resulting tokens must still be normalized into a validated array before invocation.

# Credentials And Test Infrastructure

- CI-only credentials may be ephemeral only when they are clearly scoped to isolated test infrastructure, such as throwaway Postgres service containers.
- Ephemeral test credentials must never be reused as application secrets, committed runtime credentials, or user-facing examples.
- Do not log secrets or secret-like values. Mask or omit them.

# Drift Control

- Any change to a workflow, release script, setup action, `justfile`, or `sonar-project.properties` must review the matching instruction file in the same change.
- Revaer enforces that rule mechanically with `just instruction-drift`, backed by `scripts/instruction-drift-check.sh`. Keep the mapping in that script aligned with this file and `AGENTS.md`.
- Keep `scripts/workflow-guardrails.sh` aligned with the live workflow policy when GitHub Actions pinning or shell-safety rules change.
- `pr.yml` and `ci.yml` must pass explicit base/head SHAs into `just instruction-drift` so pull requests and `main` pushes are checked against the real reviewed diff, not an incidental worktree state.
- Drift coverage for actions and release assets is recursive. Changes under `.github/actions/**`, `.github/workflows/**`, and `release/**` must keep matching the devops instruction update rule.
- Reusable workflows that publish images must preserve `packages: write` on the caller job because the callee cannot elevate a more restrictive token.
- Keep the Sonar PR gate blocking and decoration-based. Do not add `sonar.qualitygate.wait=true` to PR scans unless the branch-protection model cannot consume Sonar’s status directly.
