# CI Workflow Permissions Regression

- Status: Accepted
- Date: 2026-04-14
- Context:
  - The Helm publishing work merged to `main` left `.github/workflows/ci.yml` with two `permissions` keys on the `build-images` caller job.
  - GitHub Actions rejects duplicate keys at workflow-parse time, so the entire CI workflow failed before any jobs ran.
- Decision:
  - Keep the original `build-images` caller permissions block and remove the duplicate lower block so the workflow remains valid YAML and preserves the scopes required by the reusable image-build workflow.
  - Record the regression explicitly because workflow syntax failures bypass normal job-level validation and can break the default branch immediately.
- Consequences:
  - CI parses and schedules again on `main` without changing build behavior or token scope.
  - The reusable image-build flow still receives the required caller permissions, including `packages: write`.
- Follow-up:
  - Re-run GitHub Actions on the repaired workflow.
  - Continue reviewing workflow structure changes against the devops instruction file when modifying reusable workflow callers.

## Task Record

- Motivation:
  - Restore the default-branch CI workflow after GitHub rejected the merged workflow definition.
- Design notes:
  - The fix is intentionally minimal: remove only the duplicated `permissions` mapping and leave the existing higher-scope block in place because the reusable workflow already depends on those permissions.
- Test coverage summary:
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this is a workflow-definition repair only.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md` for workflow-change requirements and ADR task-record requirements.
  - Drift was found: the previous ADR text said no instruction wording change was needed even though this fix adds a reusable-workflow caller permission-map rule to `.github/instructions/devops.instructions.md`.
  - Removed that contradiction by documenting the new instruction wording explicitly and confirming the ADR catalogue and docs summary were updated for this task record.
- Risk & rollback plan:
  - Low risk because the change removes invalid duplicate YAML without changing job logic.
  - Rollback is a revert of this commit, though that would reintroduce the parse failure.
- Dependency rationale:
  - No new dependencies were added.
