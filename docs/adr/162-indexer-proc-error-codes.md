# Indexer proc error-code alignment for key lookups

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Motivation: ERD requires key-based lookups (trust_tier/media_domain/tag) to raise
    invalid_request with error_code=unknown_key; several stored procs still emitted
    *_not_found for key misses.
  - Constraints: keep error messages constant, preserve public-id not-found codes, and
    avoid changing schema or adding dependencies.
- Decision:
  - Update stored procedures to emit error_code=unknown_key for key-based misses while
    keeping *_not_found for public-id lookups.
  - Map unknown_key to TagServiceErrorKind::NotFound in the app service layer.
  - Verify existing role-based authorization checks and Torznab/system NULL-actor
    handling; no structural changes required.
  - Alternatives considered: introduce new error enums per proc or map unknown_key to
    Invalid; rejected to keep ERD-mandated codes and existing API semantics.
- Consequences:
  - Positive: consistent error-code taxonomy, ERD compliance, clearer API behavior for
    key lookups.
  - Risks/trade-offs: requires a function replacement migration; rollback requires
    reverting that migration if unexpected client behavior occurs.
- Follow-up:
  - Test coverage summary: `just ci` and `just ui-e2e` passed (npm audit still reports
    2 moderate vulnerabilities in the UI test workspace).
  - Observability: no new spans/metrics required (error surfaces unchanged).
  - Risk & rollback plan: revert migration `0069_indexer_proc_error_codes.sql` and the
    tag error mapping change if clients rely on previous error_code strings.
  - Dependency rationale: no new dependencies added; std/SQL only.
