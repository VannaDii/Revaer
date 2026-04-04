# Indexer schema JSON ban verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD bans JSON/JSONB storage for indexer data.
  - We need to confirm migrations comply before expanding API and service layers.
- Decision:
  - Verify all indexer migrations avoid JSON/JSONB column types and document the result.
  - Treat JSON/JSONB usage as a hard failure in schema reviews; any exception requires a
    future ADR and ERD update.
- Consequences:
  - The schema remains normalized and avoids opaque JSON storage.
  - Future migrations must continue to use normalized tables and enums.
- Follow-up:
  - Re-check JSON/JSONB usage whenever new migrations are added.

## Task record

- Motivation:
  - Ensure the schema adheres to the ERD prohibition on JSON/JSONB types.
- Design notes:
  - Reviewed the migration set and confirmed no JSON/JSONB column types are present.
- Test coverage summary:
  - Documentation-only confirmation; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future migrations introduce JSON types. Rollback by reverting offending migration
    and normalizing the data model.
- Dependency rationale:
  - No new dependencies added.
