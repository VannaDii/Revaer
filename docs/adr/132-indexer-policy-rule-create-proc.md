# Indexer policy rule creation procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - We need a stored procedure to create immutable policy rules that enforces ERD_INDEXERS.md invariants, including match-field/operator compatibility and value-set normalization.
  - Database mutations must be stored-procedure only, avoid JSON/JSONB, and return structured errors with constant messages.
- Decision:
  - Add a composite type for value-set items and a `policy_rule_create_v1` procedure that validates rule shape, match values, and value-set contents before inserting `policy_rule` rows.
  - Provide a stable `policy_rule_create` wrapper for versioning consistency.
- Consequences:
  - Policy rule creation is validated centrally in the database, preventing inconsistent match-value combinations and enforcing normalization limits.
  - Callers must supply only the expected match value type or value-set items; extra fields now fail fast.
- Follow-up:
  - Implement application-layer regex compilation validation using the stored `is_case_insensitive` flag.
  - Add stored-procedure tests that cover rule-type and value-set edge cases once the indexer DB test harness is available.
