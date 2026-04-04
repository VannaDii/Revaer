# Indexer per-table Notes verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - ERD_INDEXERS.md defines per-table Notes with validation rules, computed fields,
    and invariants that must be enforced in the schema or stored procedures.
  - The Phase 2 checklist requires verifying these notes against migrations/procs.
- Decision:
  - Verified schema-level invariants (generated columns, one-of constraints, ranges,
    and lowercase checks) across indexer tables and attribute tables.
  - Verified procedure-level enforcement for tag immutability, policy set cardinality
    and linkage rules, policy rule validation, search request validation, and
    canonical disambiguation ordering.
- Consequences:
  - Positive: DB constraints and stored procedures align with ERD Notes for validation
    and computed-field invariants.
  - Risk: runtime behaviors described in Notes (e.g., Torznab endpoints, import runner
    mapping) remain tracked in later phases and are not part of this schema validation.
- Follow-up:
  - Continue Phase 5–12 items for runtime behaviors and API surfaces.

## Task record

- Motivation:
  - Close the Phase 2 requirement to apply per-table Notes invariants in schema/procs.
- Design notes:
  - Schema constraints verified in migrations:
    - `crates/revaer-data/migrations/0012_indexer_core.sql`
    - `crates/revaer-data/migrations/0013_indexer_definitions.sql`
    - `crates/revaer-data/migrations/0014_indexer_instances.sql`
    - `crates/revaer-data/migrations/0016_search_profiles_torznab.sql`
    - `crates/revaer-data/migrations/0019_policy_sets.sql`
    - `crates/revaer-data/migrations/0021_connectivity_audit.sql`
    - `crates/revaer-data/migrations/0022_indexer_canonicalization.sql`
    - `crates/revaer-data/migrations/0023_indexer_search_requests.sql`
    - `crates/revaer-data/migrations/0025_indexer_conflicts_decisions.sql`
    - `crates/revaer-data/migrations/0026_indexer_user_actions.sql`
    - `crates/revaer-data/migrations/0027_indexer_telemetry_reputation.sql`
  - Stored-procedure validation coverage verified in:
    - `crates/revaer-data/migrations/0034_indexer_tag_procs.sql`
    - `crates/revaer-data/migrations/0040_indexer_policy_set_procs.sql`
    - `crates/revaer-data/migrations/0041_indexer_search_profile_procs.sql`
    - `crates/revaer-data/migrations/0042_indexer_policy_rule_create_proc.sql`
    - `crates/revaer-data/migrations/0049_indexer_canonical_disambiguation_rule_proc.sql`
    - `crates/revaer-data/migrations/0050_indexer_search_request_create_proc.sql`
    - `crates/revaer-data/migrations/0052_indexer_search_result_ingest_proc.sql`
- Test coverage summary:
  - just ci
  - just ui-e2e
- Observability updates:
  - None.
- Risk & rollback plan:
  - If a validation rule is found missing, add a follow-up migration or proc fix
    and revert this ADR/checklist entry.
- Dependency rationale:
  - No new dependencies.
