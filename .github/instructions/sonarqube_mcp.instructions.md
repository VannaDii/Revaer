---
applyTo:
  - ".github/workflows/sonar.yml"
  - "sonar-project.properties"
---

These are the repo-specific guidelines for using the SonarQube MCP server with Revaer.

# Project Defaults

- Default Sonar project key for this repository: `VannaDii_Revaer`
- If a user does not provide a project key, use `VannaDii_Revaer`.
- If a user provides a project key or seems unsure, confirm it with `search_my_sonarqube_projects` before acting.

# Tools To Use Here

- Use `get_project_quality_gate_status` to inspect overall, branch, or pull-request quality-gate status.
- Use `get_component_measures` for high-level metrics such as coverage, duplications, code smells, or hotspots.
- Use `search_sonar_issues_in_projects` to inspect open issues.
- Use `search_security_hotspots` and `show_security_hotspot` to review hotspot backlog or touched-code hotspots.
- Use `list_pull_requests` when you need Sonar pull-request identifiers for this project.
- Use `analyze_code_snippet` for local, file-scoped guidance when you have the full file content available.

# Revaer Sonar Workflow

- Revaer versions Sonar analysis scope in `sonar-project.properties`. Treat that file as the source of truth for authored-vs-generated scope, coverage exclusions, and duplication exclusions.
- Rust unit and integration tests may live under `src/**/tests*` as well as crate-level `tests/`; keep those test paths out of Sonar duplication gates so the PR quality signal stays focused on first-party production code.
- Follow the repo-wide external action versioning rule in `.github/instructions/devops.instructions.md` when editing `.github/workflows/sonar.yml`. Do not restate a conflicting Sonar-only pinning rule here.
- Revaer uses Sonar as a strict merge-control signal on pull requests. Prefer PR quality-gate status and decoration over scanner-side waiting in PR workflows.
- Use pull-request-specific quality-gate checks when the user asks whether a PR is blocked.
- New Security Hotspots on touched code must be reviewed before merge. Backlog hotspots outside touched code are tracked separately and do not automatically block unrelated work.

# Noise And Scope

- Generated or vendored paths excluded by `sonar-project.properties` are not first-party maintainability targets unless the user explicitly asks about them.
- If Sonar noise appears to come from generated or vendored files, verify whether the scope or exclusion rules need to be updated in `sonar-project.properties`.

# Expectations After Fixes

- After local fixes, do not immediately assume server-side issue search has refreshed. Sonar ingestion is asynchronous.
- Use `analyze_code_snippet` for immediate local feedback, but do not treat snippet analysis as a substitute for the repository scan.

# Troubleshooting

- SonarQube requires a user token for MCP access. If you see `Not authorized`, verify token type and server permissions.
- If project discovery fails, use `search_my_sonarqube_projects` before assuming a configuration bug.
