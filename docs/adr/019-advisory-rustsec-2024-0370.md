# Advisory RUSTSEC-2024-0370 Temporary Ignore

- Status: Accepted
- Date: 2025-02-21
- Context:
  - The workspace depends on `yew` for the UI crate, which transitively pulls `proc-macro-error`, currently flagged by advisory `RUSTSEC-2024-0370` (unmaintained).
  - The affected package is used only via the Yew compile-time macro stack; there is no direct runtime exposure, and no maintained alternative in the current Yew release line.
  - `cargo-deny` and `.secignore` both require an explicit justification and remediation plan for any ignore.
- Decision:
  - Keep the advisory ignored in `.secignore` and `deny.toml` while remaining on the current Yew release.
  - Monitor Yew’s releases and remove the ignore as soon as Yew drops the `proc-macro-error` dependency or provides a supported migration path.
  - No additional runtime mitigations are required because the dependency is build-time only.
- Consequences:
  - CI remains green while the upstream dependency is unresolved.
  - Risk persists until Yew publishes an update; we must track upstream progress to avoid stale ignores.
- Follow-up:
  - Track Yew issues/releases monthly and attempt upgrade; remove the ignore once the advisory is no longer transitive.
  - Re-run `just audit`/`just deny` after each Yew upgrade attempt to confirm the ignore can be removed.
  - If upstream stalls beyond Q2 2025, reassess UI stack alternatives or a forked patch to eliminate `proc-macro-error`.
