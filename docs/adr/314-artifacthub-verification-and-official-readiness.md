# Artifact Hub Verification And Official Readiness

- Status: Accepted
- Date: 2026-04-20
- Context:
  - The Revaer chart repository was already aligned to the owner-scoped OCI URL and Artifact Hub repository ID, but the remaining ownership and package-metadata details were still partly implicit.
  - Artifact Hub's current repository guidance requires the repository metadata to carry the repository ID for `Verified publisher`, and ownership claim flows depend on published owner identity that matches the Artifact Hub account or organization member performing the claim.
  - Artifact Hub also recommends explicit package metadata where automatic extraction may be incomplete, including chart image metadata that powers package security scanning.
- Decision:
  - Keep `charts/revaer/artifacthub-repo.yml` as the canonical repository-metadata template and document that owner identity must match the Artifact Hub claimant.
  - Update `release/scripts/helm-package.sh` so owner metadata is appended whenever `ARTIFACTHUB_OWNER_NAME` and `ARTIFACTHUB_OWNER_EMAIL` are available, including unsigned packaging paths.
  - Publish an explicit `artifacthub.io/images` chart annotation at release packaging time using the Revaer GHCR image tag that matches the chart app version.
  - Refresh the chart README, release checklist, and devops instructions to record the remaining manual Artifact Hub steps: public GHCR visibility, repository add/claim, verified-publisher confirmation, and the manual `official` status request.
- Consequences:
  - Published Artifact Hub repository metadata is now authoritative for both repository verification and ownership claim workflows instead of depending on signing-only paths.
  - Artifact Hub can index the chart's primary runtime image from chart metadata even if automatic image extraction is incomplete.
  - The `official` badge still cannot be granted from Git alone; the repository can only be made ready for that manual Artifact Hub request.
- Follow-up:
  - Re-run a Helm publish and confirm the `artifacthub.io` OCI metadata artifact contains `repositoryID` plus the expected owner entry.
  - Confirm the next Artifact Hub processing cycle shows `Verified publisher`, then submit the `official` status request if it has not already been filed.

## Task Record

- Motivation:
  - Make the repository metadata authoritative enough for Artifact Hub verification and official-status workflows instead of leaving those steps partially dependent on operator memory or signing-only side effects.
- Design notes:
  - Owner identity stays externally configurable through `ARTIFACTHUB_OWNER_*`, with GPG UID fallback retained for signed releases.
  - The chart image annotation is injected at packaging time so the published image tag stays aligned with the release tag or prerelease tag.
- Test coverage summary:
  - `just helm-lint`
  - `just instruction-drift`
- Observability updates:
  - No runtime observability surface changed.
  - Artifact Hub package metadata now exposes the published runtime image more reliably for external scanning and UI display.
- Status-doc validation:
  - Re-checked `charts/revaer/README.md`, `docs/release-checklist.md`, `.github/instructions/devops.instructions.md`, `docs/adr/index.md`, and `docs/SUMMARY.md`; updated them to match the Artifact Hub readiness flow.
- Risk & rollback plan:
  - Main risk is stale or incorrect `ARTIFACTHUB_OWNER_*` workflow variables causing an ownership mismatch in published metadata. Roll back by correcting the variables and republishing the chart metadata artifact.
  - If the explicit image annotation proves incorrect for a future image-layout change, remove or revise the injected annotation and republish.
- Dependency rationale:
  - No new dependencies were added.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - Drift found: the instruction and operator docs did not yet state that Artifact Hub owner identity must remain present outside signing-only paths, and they did not record the explicit manual steps needed for `official` readiness.
  - Removed that drift by updating the release script, docs, and instruction file together.
