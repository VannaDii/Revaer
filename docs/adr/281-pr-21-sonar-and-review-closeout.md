# PR 21 Sonar and Review Closeout

- Status: Accepted
- Date: 2026-04-04
- Context:
  - PR 21 still had open SonarCloud feedback on the leak period after the earlier remediation follow-up landed.
  - The remaining Sonar issues were limited to GitHub Actions security hotspots in the image build workflow and new-code duplication in the filesystem post-processing service.
- Decision:
  - Pin the flagged GitHub Actions steps in `build-images.yml` to immutable full commit SHAs.
  - Refactor the duplicated archive-extraction and tree-transfer logic in `revaer-fsops` into shared helpers without changing runtime behavior.
- Consequences:
  - The workflow now follows the immutable action-pin guidance Sonar was flagging on the PR delta.
  - The fsops module has less repeated code, which lowers Sonar duplication noise and makes future archive and transfer changes easier to review.
- Follow-up:
  - Re-run the full `just ui-e2e` and `just ci` gates before hand-off.
  - Push the branch so GitHub and SonarCloud can recalculate PR 21 status against the updated head commit.

## Task Record

- Motivation:
  - Close the last open PR 21 review findings and SonarCloud leak-period issues so the remediation branch can merge without lingering security or maintainability flags.
- Design notes:
  - `build-images.yml` keeps the same action versions semantically, but now pins the exact commits behind the previously version-tagged actions.
  - `revaer-fsops` now has shared helpers for archive write operations, relative-path normalization, and directory-tree replication, which removes the repeated zip/tar and copy/hardlink blocks Sonar was reporting.
  - The UI Playwright fixture now seeds auth through an in-memory storage shim instead of writing API keys into browser storage, which closes the remaining GitHub Advanced Security review threads on `tests/fixtures/app.ts`.
  - The refactor stayed behavior-preserving and reuses the existing fsops test coverage for archive extraction, checksum generation, and file transfer behavior.
- Test coverage summary:
  - `just fmt`
  - `just lint`
  - `just ui-e2e`
  - `just ci`
- Observability updates:
  - No new metrics or spans were needed.
  - Existing fsops metric emission remains unchanged because the work only reshaped helper internals and workflow pins.
- Status-doc validation:
  - `README.md` and the existing remediation status docs were re-checked; no operator-facing behavior changed, so no status-doc content updates were required beyond this task record and catalogue entries.
- Risk & rollback plan:
  - Workflow pinning risk is limited to an incorrect SHA; rollback is a revert of the workflow pin lines.
  - Fsops refactor risk is confined to archive extraction and transfer helpers; rollback is a revert of `crates/revaer-fsops/src/service/mod.rs`.
- Dependency rationale:
  - No new dependencies were added.
  - The duplication cleanup deliberately reused `std` and the existing crate graph instead of introducing helper crates or archive abstractions.
