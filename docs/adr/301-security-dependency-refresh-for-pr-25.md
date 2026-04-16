# Security Dependency Refresh For PR 25

- Status: Accepted
- Date: 2026-04-16
- Context:
  - PR 25 was failing `Run Audit` on new `rustls-webpki` advisories and `Check Deny` on stale exception state.
  - The repository also carried an older `RUSTSEC-2026-0097` exception that needed to be re-evaluated against the live dependency graph rather than left untouched.
- Decision:
  - Update `rustls-webpki` to `0.103.12` and refresh the `rand 0.9` line to `0.9.4` in `Cargo.lock`.
  - Keep the `cargo audit` ignore for `RUSTSEC-2026-0097` only in `.secignore`, because `rand 0.8.5` still arrives transitively through `sqlx-postgres`.
  - Remove the stale `cargo-deny` advisory ignore for `RUSTSEC-2026-0097` and update the duplicate-version skip entry from `rand@0.9.2` to `rand@0.9.4`.
- Consequences:
  - The PR's audit failures for `RUSTSEC-2026-0098` and `RUSTSEC-2026-0099` are cleared by dependency refresh instead of by adding new ignores.
  - The old `rand` advisory exception is narrowed to the remaining unresolved `sqlx-postgres` path instead of covering both old and new `rand` branches.
  - `cargo-deny` no longer carries an unmatched advisory ignore or an outdated duplicate-version skip for `rand 0.9.2`.
- Follow-up:
  - Keep monitoring `sqlx` updates for a release that removes the remaining `rand 0.8.5` path.
  - Remove `RUSTSEC-2026-0097` from `.secignore` once the workspace no longer resolves that version.

## Task Record

- Motivation:
  - Restore PR 25's failing audit/deny checks by updating dependencies where compatible fixes exist and cleaning up stale security exceptions.
- Design notes:
  - The dependency refresh was intentionally limited to lockfile-compatible updates that the existing manifests can absorb without a broader dependency migration.
  - `postgres-protocol` was tested and then reverted because it introduced unnecessary duplicate-crate churn without solving the remaining `rand 0.8.5` advisory path.
- Test coverage summary:
  - Reran `cargo audit` with the live ignore set.
  - Reran `cargo deny check`.
  - Reran `just ci`.
  - Reran `just ui-e2e`.
- Observability updates:
  - No runtime observability surfaces changed; this is dependency and policy maintenance only.
- Stale-policy check:
  - Reviewed `AGENTS.md`, `.secignore`, and `deny.toml`.
  - Drift was found: `deny.toml` still ignored `RUSTSEC-2026-0097` even though `cargo-deny` no longer detected that advisory, and it still skipped `rand@0.9.2` after the lockfile moved to `rand@0.9.4`.
  - Removed those stale exception details and updated the remaining audit ignore comment to document the actual unresolved `sqlx-postgres` path.
- Risk & rollback plan:
  - Risk is limited to dependency-resolution regressions from lockfile updates and stricter security check posture.
  - Rollback is a revert of the lockfile and exception-file changes if they destabilize CI unexpectedly.
- Dependency rationale:
  - No new first-party dependencies were added.
  - Lockfile refreshes were preferred over adding fresh ignores because fixed compatible releases already existed for `rustls-webpki` and the `rand 0.9` branch.
