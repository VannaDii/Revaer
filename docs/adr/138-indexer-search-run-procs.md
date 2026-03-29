# Indexer search run procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Search runs need explicit state transitions and retry backoff rules per ERD_INDEXERS.md.
  - Retryable failures and rate-limited deferrals must keep runs queued while enforcing limits.
- Decision:
  - Add stored procedures for enqueue, start, finish, fail, and cancel of search indexer runs.
  - Implement backoff calculations for retryable errors and rate-limited deferrals inside the database.
- Consequences:
  - Run state transitions are validated in one place and aligned with status timestamp constraints.
  - Coordinators must pass retry_seq and rate-limit scope to ensure correct backoff.
- Follow-up:
  - Implement `search_request_create_v1` to seed runs and policy snapshots.
  - Wire outbound request logging to update run correlation state.
