# Semantic Release Prepare Template Fix

- Status: Accepted
- Date: 2026-04-18
- Context:
  - The `Publish Dev Release` job in GitHub Actions failed on April 17, 2026 in run `24586113873`, job `71897294770`, during the semantic-release `prepare` step.
  - `release/release.config.js` embedded Bash parameter expansion syntax inside `@semantic-release/exec` `prepareCmd`, but that field is first rendered through lodash templates.
  - The `${REVAER_ENABLE_HELM_RELEASE_ASSETS:-0}` fragment was parsed as a template expression, causing `SyntaxError: Unexpected token ':'` before the shell command ran.
- Decision:
  - Replace the parameter-expansion form with a plain quoted environment-variable comparison that semantic-release leaves untouched.
  - Keep the Helm packaging behavior gated by `REVAER_ENABLE_HELM_RELEASE_ASSETS` so prerelease packaging still happens only in the intended workflow path.
- Consequences:
  - Dev release preparation no longer fails during template rendering.
  - Unset `REVAER_ENABLE_HELM_RELEASE_ASSETS` still skips Helm packaging because an empty string does not match `"1"`.
  - The release flow stays dependency-neutral and keeps the existing shell-based packaging contract.
- Follow-up:
  - Keep shell syntax inside semantic-release command templates free of `${...}` forms unless they are semantic-release placeholders.
  - Revisit other release command templates if more shell interpolation is added later.

## Task Record

- Motivation:
  - Restore the failing `main` release workflow with the smallest safe change that matches the logged failure.
- Design notes:
  - The fix is limited to `release/release.config.js`; it preserves the existing `write-release-info` and Helm packaging order and only changes the environment-variable check syntax.
- Test coverage summary:
  - Reran a semantic-release dry run locally against `release/release.config.js`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this work only repairs release automation.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.github/instructions/devops.instructions.md`, `.github/instructions/rust.instructions.md`, `justfile`, and `release/release.config.js`.
  - Drift was found: the release configuration violated the documented semantic-release prepare-phase contract because template-hostile shell syntax prevented the command from executing.
  - Removed that contradiction by switching the gate to a template-safe environment-variable comparison without changing the release workflow contract.
- Risk & rollback plan:
  - Risk is limited to dev release packaging; if the new condition were mistyped, Helm assets could be skipped unexpectedly.
  - Rollback is a revert of this commit and restoration of the prior release config once an alternative template-safe gating strategy is ready.
- Dependency rationale:
  - No new dependencies were added.
  - The fix stays inside the existing semantic-release configuration instead of adding wrapper scripts or release plugins.
