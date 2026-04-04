# Indexer definition schema

- Status: Accepted
- Date: 2026-01-25
- Context:
  - The indexer ERD requires a catalog of indexer definitions and field metadata.
  - These tables are prerequisites for indexer instance configuration and import flows.
- Decision:
  - Add a migration that introduces indexer definition enums and tables, including
    validation rules and value sets.
  - Encode ERD constraints as database checks and unique indexes where possible.
- Consequences:
  - Positive: definition metadata can be stored and validated at the database layer.
  - Trade-off: adds a new migration that must be extended by later ERD stages.
- Follow-up:
  - Add indexer instance tables and import flows.
  - Implement seed and stored-procedure logic for definition sync.

## Task record

- Motivation:
  - Continue the dependency-first ERD rollout with the catalog and validation schema.
- Design notes:
  - Enum types are created idempotently via pg_type checks.
  - Validation rules are enforced with explicit CHECK constraints and a unique index.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if downstream schemas change.
- Dependency rationale:
  - No new dependencies added.
