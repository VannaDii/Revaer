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
- External GitHub actions in modified files must be pinned by full commit SHA. Keep a version comment alongside the SHA for readability.
- Workflow build, lint, test, coverage, and release gates must call `just` recipes. Do not reintroduce raw `cargo` pipelines into CI jobs.
- Treat `sonar-project.properties` as the versioned source of truth for Sonar analysis scope and exclusions.

# Shell Safety

- Never interpolate untrusted `${{ inputs.* }}` or comparable expression values directly into `run:` blocks.
- Map user-controlled inputs into environment variables first, validate or whitelist them, then consume them in shell.
- Prefer arrays and quoted expansions over word-splitting command strings.

# Credentials And Test Infrastructure

- CI-only credentials may be ephemeral only when they are clearly scoped to isolated test infrastructure, such as throwaway Postgres service containers.
- Ephemeral test credentials must never be reused as application secrets, committed runtime credentials, or user-facing examples.
- Do not log secrets or secret-like values. Mask or omit them.

# Drift Control

- Any change to a workflow, release script, setup action, `justfile`, or `sonar-project.properties` must review the matching instruction file in the same change.
- Keep the Sonar PR gate blocking and decoration-based. Do not add `sonar.qualitygate.wait=true` to PR scans unless the branch-protection model cannot consume Sonar’s status directly.
