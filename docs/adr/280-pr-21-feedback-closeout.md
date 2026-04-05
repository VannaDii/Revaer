# PR 21 feedback closeout

- Status: Accepted
- Date: 2026-04-04
- Context:
  - PR 21 still had unresolved review threads covering qB metadata sync behavior, fsops checksum manifest accounting, runbook artifact retention, and UI auth/E2E stability.
  - The follow-up needed to address the reviewer asks directly, keep the remediation branch shippable, and restore the required `just ui-e2e` and `just ci` gates before the PR could move forward.
- Decision:
  - Close the review threads with targeted fixes that map one-to-one to the remaining comments.
  - Treat the unstable UI suite as part of the review scope because the updated auth storage behavior and shared E2E backend needed deterministic coverage before the PR could be considered ready.
- Consequences:
  - qB metadata-only mutations now publish compatibility sync updates, checksum manifest metadata reports real manifest byte counts, and the runbook preserves Playwright artifacts on failure.
  - UI E2E now seeds auth into session storage with matching read fallback, uses deterministic log-filter interactions, aligns stale route assertions with the implemented UI, and defaults to a single UI worker unless the environment overrides it.
- Follow-up:
  - Keep watching the Playwright worker override path in CI or faster hosts to ensure the serial default remains the right trade-off.
  - Remove any future stale UI assertions as the pages evolve instead of pinning tests to old placeholder copy.

## Task Record

- Motivation:
  - The PR had unresolved actionable review comments and could not be handed back until both the requested fixes and the repo quality gates were green.
- Design notes:
  - qB metadata updates were routed through a shared helper so each metadata mutation publishes the same compatibility refresh event.
  - Fsops checksum manifest accounting now derives manifest bytes from the serialized manifest lines instead of placeholder counts.
  - The UI fixture now seeds auth in the same storage tier the browser session should own, while the app preferences layer reads both local and session storage to stay backward compatible during transition.
  - The Playwright suite now defaults to one UI worker because the UI tests share a mutable backend and a single trunk-served frontend process; `E2E_UI_WORKERS` still allows explicit overrides.
- Test coverage summary:
  - Reran `just ui-e2e` successfully with `101 passed`.
  - Reran `just ci` successfully after the feedback fixes landed.
- Observability updates:
  - qB metadata-only compatibility mutations now emit sync-visible event updates instead of silently mutating state.
  - The runbook now preserves logs, Playwright reports, and test-results artifacts even on failure.
- Status-doc validation:
  - No README or roadmap status claims changed in this follow-up.
  - ADR catalogue entries were updated to record this task.
- Risk & rollback plan:
  - The highest-risk change is the UI E2E worker default. If it regresses on faster environments, rollback is limited to the Playwright config default while preserving the explicit override hook.
  - The qB/fsops/runbook changes are localized and can be reverted independently if they cause regressions.
- Dependency rationale:
  - No new dependencies were added.
