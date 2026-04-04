# Indexer allocation safety guard

- Status: Accepted
- Date: 2026-02-01
- Context:
  - Motivation: Prevent unbounded allocations in indexer handlers and satisfy security review feedback.
  - Constraints: No new dependencies; errors must use constant messages with structured context.
- Decision:
  - Add a shared allocation helper that reads `MemAvailable` from `/proc/meminfo` and limits
    requested allocations to 80% of available memory.
  - Apply the helper to dynamic list normalization in search profiles, policy rules, and media
    domain allowlists, while raising per-list caps to avoid overly constraining users.
  - Dependency rationale: none (std-only implementation).
- Consequences:
  - Positive outcomes: safer allocations, explicit error reporting with context, consistent limits.
  - Risks or trade-offs: allocation checks fail closed if `MemAvailable` cannot be read; rollback by
    relaxing the guard to a fixed ceiling if needed.
- Follow-up:
  - Implementation tasks: add helper module, update normalization paths, add unit tests.
  - Test coverage summary: unit tests for allocation guard and meminfo parsing.
  - Observability updates: none required; errors carry context fields for diagnostics.
