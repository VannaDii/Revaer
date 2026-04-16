# Trivy Container And Sonar PGSQL Config

- Status: Accepted
- Date: 2026-04-16
- Context:
  - Revaer now has a root `trivy.yaml`, but it only expressed generic scanner and severity settings.
  - The repository's Sonar configuration documents PostgreSQL migration noise, yet it did not explicitly map PostgreSQL-oriented file suffixes such as `.pgsql` and `.plpgsql` into Sonar's available SQL analyzer path.
- Decision:
  - Extend `trivy.yaml` with explicit container-image settings so image scans prefer remote registry artifacts, inspect both OS and library packages, and include image misconfiguration checks alongside vulnerability and secret scanning.
  - Update `sonar-project.properties` to keep `.sql` mapped to PL/SQL and explicitly add `.pgsql` and `.plpgsql` suffixes, while leaving the existing PostgreSQL-noise exclusions and ignored-rule posture in place.
- Consequences:
  - Trivy's checked-in baseline now describes the container-image behavior Revaer expects instead of relying on image-command defaults alone.
  - Sonar remains best-effort for PostgreSQL stored procedures, but PostgreSQL-specific suffixes are now discoverable by analysis without pretending SonarCloud has a native PostgreSQL dialect mode.
- Follow-up:
  - Re-run Trivy-backed image scans after workflow execution to confirm the container baseline behaves as expected.
  - Revisit Sonar SQL scope if SonarCloud adds PostgreSQL-aware analysis that can replace the PL/SQL suffix-mapping workaround.

## Task Record

- Motivation:
  - Make the repo's Trivy and Sonar SQL behavior explicit for container images and PostgreSQL procedure files.
- Design notes:
  - `trivy.yaml` now codifies image-source preference and package/image scan scope while still allowing workflow steps to override output and exit handling.
  - Sonar suffix mapping stays conservative: `.sql`, `.pgsql`, and `.plpgsql` are routed into the existing PL/SQL analyzer because that is the only available analyzer path documented for this setup.
- Test coverage summary:
  - Verified locally that Trivy `v0.69.3` loads `trivy.yaml`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this is repository scan-configuration maintenance only.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/instructions/devops.instructions.md`, and `.github/instructions/sonarqube_mcp.instructions.md`.
  - Drift was found in the Sonar instruction set: it did not state the repository's explicit PostgreSQL suffix-mapping rule.
  - Removed that gap by adding the PostgreSQL suffix-mapping guidance to `.github/instructions/sonarqube_mcp.instructions.md`.
- Risk & rollback plan:
  - Risk is limited to CI/static-analysis signal changes from broader Trivy image scanning and more explicit Sonar SQL suffix routing.
  - Rollback is a revert of `trivy.yaml`, `sonar-project.properties`, and this ADR if scan noise or compatibility regresses.
- Dependency rationale:
  - No new project dependencies were added.
