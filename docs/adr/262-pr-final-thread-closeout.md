# PR final thread closeout

- Status: Accepted
- Date: 2026-03-28
- Context:
  - Pull request 6 still had two unresolved, non-outdated review threads after the earlier security and handler cleanup passes.
  - One thread targeted the noisy `router.rs` import surface for indexer handlers, and the other targeted the large test-only `ErrorIndexers` stub in the secrets handler tests.
  - We needed to close those threads without reopening broader behavior or security review.
- Decision:
  - Collapse the router dependency surface to the indexer handler module boundary by importing `crate::http::indexers` once and qualifying route handlers through that module.
  - Reuse the shared `RecordingIndexers` test double for secrets handler failure-path tests by adding a focused `secret_error` injection point instead of maintaining a trait-wide `ErrorIndexers` implementation.
  - Keep the rest of the behavior unchanged and validate with targeted handler tests plus the full `just ci` and `just ui-e2e` gates.
- Consequences:
  - The router is less noisy and less likely to incur merge conflicts when indexer handler exports change.
  - Secrets handler tests no longer carry a large maintenance burden each time `IndexerFacade` grows.
  - Test support now owns one more injectable error path, which modestly expands the shared fixture surface but keeps it centralized.
- Follow-up:
  - Update PR #6 discussion replies and resolve the remaining fixed threads directly on GitHub.
  - Keep using shared handler test support instead of bespoke trait stubs when future indexer handler tests need error injection.
