# Indexer audit fields and timestamp defaults verification

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The ERD requires audit fields (created/updated/changed by) to be non-null where used
    and mandates `created_at`/`updated_at` defaults when those columns exist.
  - We need to confirm the indexer schema matches these requirements before expanding APIs.
- Decision:
  - Verify audit fields are present and non-null where required, and timestamp defaults are
    set on indexer tables that include `created_at`/`updated_at`.
  - Confirmed examples (migrations 0012–0023):
    - Audit fields:
      - `tag`, `routing_policy`, `indexer_instance`, `search_profile`, `policy_set`,
        `policy_rule` include `created_by_user_id`/`updated_by_user_id` as NOT NULL.
      - `indexer_instance_field_value` includes `updated_by_user_id` as NOT NULL.
      - `canonical_disambiguation_rule` includes `created_by_user_id` as NOT NULL.
      - `config_audit_log` includes `changed_by_user_id` as NOT NULL.
    - Timestamp defaults:
      - Tables with `created_at`/`updated_at` columns define them as NOT NULL DEFAULT now(),
        including `tag`, `routing_policy`, `indexer_instance`, `search_profile`,
        `policy_set`, `policy_rule`, `canonical_torrent`, `canonical_torrent_source`,
        `torznab_instance`, and `rate_limit_policy`.
- Consequences:
  - Schema audit columns are enforced consistently and can be trusted by API and UI layers.
  - Timestamp defaults align with ERD expectations for lifecycle tracking.
- Follow-up:
  - Re-verify audit/timestamp columns for any new indexer migrations.

## Task record

- Motivation:
  - Establish that audit fields and lifecycle timestamps are enforced per the ERD.
- Design notes:
  - Verified audit field presence and NOT NULL constraints in migrations 0012, 0014,
    0016, 0019, 0021, and 0022.
  - Verified created_at/updated_at defaults in the same migration set.
- Test coverage summary:
  - Documentation-only verification; no new tests added.
- Observability updates:
  - None.
- Risk & rollback plan:
  - Risk: future tables omit audit fields or defaults. Roll back by correcting the schema
    migration and revalidating against the ERD.
- Dependency rationale:
  - No new dependencies added.
