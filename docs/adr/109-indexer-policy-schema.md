# Indexer policy schema

- Status: Accepted
- Date: 2026-01-26
- Context:
  - ERD defines policy sets, rules, and snapshots for search filtering and scoring.
  - Search profiles need policy_set linkage for profile-scoped policies.
- Decision:
  - Add policy_set, policy_rule, policy_rule_value_set, policy_rule_value_set_item,
    policy_snapshot, policy_snapshot_rule, and search_profile_policy_set tables.
  - Introduce required policy enums and enforce ERD uniqueness and cascade rules.
- Consequences:
  - Positive: schema supports policy configuration, snapshot reuse, and profile links.
  - Trade-off: stored procedures and snapshot materialization remain follow-up work.
- Follow-up:
  - Implement policy procedures, snapshot hashing, and retention jobs per ERD.
  - Add search_request tables to wire policy snapshots into runtime queries.

## Task record

- Motivation:
  - Continue ERD implementation with policy persistence and profile linkage.
- Design notes:
  - policy_set created_for_search_request_id is stored without a FK until search_request exists.
  - policy_rule_value_set uses shared value_set_type enum without extra restrictions.
- Test coverage summary:
  - No new tests added; migration path is exercised via just ci and ui-e2e.
- Observability updates:
  - None in this change.
- Risk & rollback plan:
  - Roll back by reverting the migration if ERD constraints change.
- Dependency rationale:
  - No new dependencies added.
