# Indexer executor handoff stored procedures

- Status: Accepted
- Date: 2026-01-27
- Context:
  - External executor work (RSS polling and indexer test probes) must be orchestrated
    through stored procedures, with clear concurrency control and auditable outcomes.
  - The ERD requires separate claim/apply phases so the database remains the single
    source of truth while network calls run outside the DB.
  - Secrets must remain encrypted at rest and only surfaced to the executor via
    explicit read procedures.
- Decision:
  - Add RSS polling claim/apply procedures and indexer test prepare/finalize procedures.
  - Provide a secret read procedure for executor access, allowing system callers to pass
    a NULL actor while still enforcing revocation checks.
  - Keep procedure inputs/outputs aligned with ERD contract and use outbound_request_log
    for telemetry.
  - Alternatives considered:
    - Single procedure that performs polling/tests and logging inside the DB.
    - Executor-side direct table access without stored procedures.
- Consequences:
  - Positive outcomes:
    - Clear concurrency boundaries for polling/test work with SKIP LOCKED claims.
    - Consistent logging and scheduling semantics driven by the ERD.
  - Risks or trade-offs:
    - Requires executor code to implement the two-phase workflow and handle retries.
    - Adds more stored procedure surface area to maintain.
- Follow-up:
  - Implement migrations for RSS poll claim/apply, indexer test prepare/finalize,
    and secret read procedures.
  - Update checklist tracking and verify integration tests once executor wiring lands.
