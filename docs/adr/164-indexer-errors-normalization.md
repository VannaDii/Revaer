# Indexer error enums and normalization helpers verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: Phase 6 requires per-crate error enums with constant messages and
    context fields, plus normalization helpers for hashing and magnet/title inputs.
  - Constraints: preserve existing stored-procedure boundaries and avoid new
    dependencies.
- Decision:
  - Verified error enums and constant-message patterns for indexer paths across
    `revaer-data` (`DataError`), `revaer-app` (`AppError`), and `revaer-api`
    (`TagServiceError`).
  - Verified normalization helpers and wrappers in `revaer-data/src/indexers/normalization.rs`
    and the supporting stored procedures (`normalize_title`, `normalize_magnet_uri`,
    `derive_magnet_hash`, `compute_title_size_hash`).
  - Alternatives considered: introducing new error enums or normalization helpers in
    additional crates; rejected because current coverage meets ERD requirements.
- Consequences:
  - Positive: checklist items are satisfied without new dependencies or API changes.
  - Risks/trade-offs: future indexer services must keep the same constant-message +
    context-field pattern to remain compliant.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` passed (npm audit still reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: no additional spans/metrics needed for this verification step.
  - Risk & rollback plan: documentation-only change; revert ADR and checklist updates
    if verification is found incomplete.
  - Dependency rationale: no new dependencies added.
