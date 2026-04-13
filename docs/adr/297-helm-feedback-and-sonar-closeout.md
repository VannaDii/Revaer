# Helm Feedback And Sonar Closeout

- Status: Accepted
- Date: 2026-04-13
- Context:
  - PR 23 added Helm packaging and publishing, then picked up follow-up review comments and 21 Sonar shell issues in the release scripts.
  - The release flow already relied on signed chart artifacts and separate Artifact Hub repository metadata, so the cleanup needed to preserve that contract rather than redesign it.
- Decision:
  - Harden the Helm shell scripts in place by adopting explicit Bash conditionals, clearer helper-local variables, and explicit helper returns where Sonar flagged maintainability issues.
  - Tighten the release path by excluding `artifacthub-repo.yml` from packaged chart tarballs, exporting temporary secret key material with owner-only permissions, and verifying `.tgz` plus `.prov` artifacts before OCI publication.
- Consequences:
  - The Helm release path remains aligned with the original design but now satisfies current PR review feedback and Sonar shell-quality expectations.
  - Publishing is slightly stricter: missing provenance or keyring assets now fail the publish step instead of allowing an unsigned chart push.
- Follow-up:
  - Let GitHub Actions and SonarCloud rescan PR 23 after the branch update.
  - Keep future Helm script changes aligned with `.github/instructions/devops.instructions.md` so instruction-drift stays explicit.

## Task Record

- Motivation:
  - Clear the remaining PR review comments and remove the new-code Sonar findings on the Helm release work before merge.
- Design notes:
  - Added `.helmignore` rather than moving repository metadata out of the chart tree, because the packaging flow already copies the chart directory and Helm natively supports excluding non-chart files.
  - Kept provenance verification in `helm-publish.sh` so both prerelease and stable publication paths enforce the same signed-artifact contract.
- Test coverage summary:
  - Reran `just helm-lint`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime logging, tracing, metrics, or health-surface changes were introduced; this work is limited to release automation and chart packaging hygiene.
- Status-doc validation:
  - Re-checked the Helm release instruction surface in `.github/instructions/devops.instructions.md` and updated it to match the tightened packaging and publish behavior.
  - Updated ADR indexes so the task record is discoverable from the docs navigation.
- Risk & rollback plan:
  - Main risk is over-constraining release packaging if expected provenance assets are missing. Rollback is a revert of this closeout commit, restoring the prior packaging behavior.
  - The permission hardening and `.helmignore` changes are low-risk because they narrow artifact contents and file exposure rather than widening behavior.
- Dependency rationale:
  - No new dependencies were added. The changes reuse existing Bash, Helm, GPG, and ORAS tooling already required by the Helm release flow.
