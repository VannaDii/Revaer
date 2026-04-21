# Artifact Hub OCI Repository Alignment

- Status: Accepted
- Date: 2026-04-20
- Context:
  - The PR Helm workflow was publishing charts to `ghcr.io/<owner>/<repo>/charts/revaer`, while the Artifact Hub repository that now exists is configured for `oci://ghcr.io/vannadii/charts/revaer`.
  - That namespace mismatch meant successful workflow publishes would land in GHCR, but not at the OCI repository URL Artifact Hub is actually tracking.
  - The Artifact Hub repository now has the stable ID `dfbc5c47-d0c5-4ac7-b9d4-5812c0a6a15a`, which needs to be present in the published repository metadata for verified ownership workflows.
- Decision:
  - Change the default GHCR Helm namespace derivation to publish charts to `ghcr.io/<owner>/charts/revaer`.
  - Ship the Artifact Hub repository ID in `charts/revaer/artifacthub-repo.yml` and keep release packaging from appending a duplicate `repositoryID`.
  - Refresh install and release docs so they reference the owner-scoped OCI chart URL rather than the older repo-scoped path.
- Consequences:
  - PR, `main`, tag, and manual Helm publishes now target the same OCI repository URL that Artifact Hub is configured to ingest.
  - Artifact Hub repository verification metadata is stable even when GitHub Actions repository variables are unset.
  - Existing references to the repo-scoped GHCR path become stale and must be updated together when the public OCI location changes.
- Follow-up:
  - Push a fresh PR Helm publish and confirm new versions appear under `ghcr.io/vannadii/charts/revaer`.
  - Re-check Artifact Hub after its next repository processing cycle and confirm it indexes the newly published PR prerelease.

## Task Record

- Motivation:
  - Align the workflow's actual Helm publish destination with the Artifact Hub repository the user created so PR dev chart publishes become visible in Artifact Hub.
- Design notes:
  - `release/scripts/helm-publish.sh` now derives the default namespace from the GitHub owner only, because the chart name is already appended as `/revaer`.
  - `charts/revaer/artifacthub-repo.yml` carries the canonical repository ID and marks the repo as `oci`; `helm-package.sh` avoids duplicating that field when env overrides are also present.
- Test coverage summary:
  - `just instruction-drift`
  - `bash scripts/workflow-guardrails.sh`
  - `just ui-e2e`
  - `just ci`
- Observability updates:
  - No runtime observability surface changed.
  - The externally visible change is the GHCR package location and matching Artifact Hub metadata target.
- Status-doc validation:
  - Re-checked `charts/revaer/README.md`, `docs/release-checklist.md`, `.github/instructions/devops.instructions.md`, `docs/adr/index.md`, and `docs/SUMMARY.md`; updated stale GHCR path references.
- Risk & rollback plan:
  - The main risk is consumers still pulling from the old repo-scoped GHCR chart path. Roll back by restoring the previous namespace derivation and reverting the docs if the owner-scoped repository proves incompatible.
  - Artifact Hub ingestion remains asynchronous, so validation must allow for the service's reprocessing delay after publish.
- Dependency rationale:
  - No new dependencies were added.
- Stale-policy check:
  - Reviewed `AGENTS.md` and `.github/instructions/devops.instructions.md`.
  - Drift found: devops instructions and release docs did not state the owner-scoped public OCI chart location or the chart metadata file as the repository-ID source of truth.
  - Removed that drift by updating the instruction file, release docs, and ADR index in this change.
