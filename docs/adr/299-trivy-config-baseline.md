# Trivy Config Baseline

- Status: Accepted
- Date: 2026-04-16
- Context:
  - Revaer's image scan workflow uses Trivy, but the repository had no root `trivy.yaml`.
  - Trivy automatically reads `trivy.yaml` from the current working directory, so keeping a repo-local baseline config makes the scan policy explicit and reusable across local and CI invocations.
- Decision:
  - Add a root `trivy.yaml` that encodes Revaer's baseline Trivy scan posture.
  - Keep the baseline conservative and aligned with existing image-scan behavior by scanning for vulnerabilities and secrets, restricting findings to `HIGH` and `CRITICAL`, and leaving unfixed vulnerabilities visible.
- Consequences:
  - The repository now has a valid Trivy configuration file that local invocations and CI can share.
  - Workflow steps can still override output format, SARIF path, and exit-code behavior without forking the underlying baseline policy.
- Follow-up:
  - Re-run Trivy-backed image scans against the repository workflows.
  - Keep `trivy.yaml` aligned with future workflow policy changes if scan scope or severity thresholds change.

## Task Record

- Motivation:
  - Make Trivy configuration explicit in-repo instead of relying on implicit defaults only.
- Design notes:
  - The config intentionally mirrors the repo's current image-scan posture rather than broadening coverage or altering CI failure conditions.
  - Report formatting and exit behavior were left out of `trivy.yaml` because the reusable image workflow already sets those per job.
- Test coverage summary:
  - Validated the config structure against Trivy's published configuration-file schema and option names.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this is repository scan-policy configuration only.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - No instruction drift was found that required a wording change for this config-only addition.
  - Updated the ADR catalogue and docs summary for the new task record.
- Risk & rollback plan:
  - Low risk because the file only codifies the existing Trivy baseline and workflow steps can still override job-specific reporting behavior.
  - Rollback is a revert of this ADR and `trivy.yaml` if a future Trivy release requires a different config shape.
- Dependency rationale:
  - No new dependencies were added.
