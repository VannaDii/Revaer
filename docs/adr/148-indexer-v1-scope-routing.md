# Indexer v1 scope enforcement

- Status: Accepted
- Date: 2026-01-27
- Context:
  - The indexer ERD defines explicit v1 scope and non-goals that must guide architecture and
    route planning.
  - The implementation plan needs a clear guardrail so API/UI work does not drift into
    media management or other out-of-scope features.
- Decision:
  - Confirm that indexer v1 architecture and route planning are constrained to the ERD scope:
    - Indexers, search, policies, secrets, routing, rate limiting, Torznab compatibility, and
      reliability/telemetry flows are in scope.
    - Media management features remain out of scope for v1 and require a future ADR before
      any routes or services are added.
  - Document the scope rule as a checklist gate and require any scope expansion to add a new
    ADR and update ERD_INDEXERS.md.
- Consequences:
  - Implementation stays aligned with the ERD and avoids premature media management APIs.
  - Route planning focuses on indexer and search workflows with explicit boundaries.
- Follow-up:
  - Keep ERD_INDEXERS_CHECKLIST.md in sync with any scope changes.
  - Add ADRs for any new surfaces that expand beyond v1 scope.

## Task record

- Motivation:
  - Prevent scope creep and ensure indexer architecture and route planning remain consistent
    with v1 goals and non-goals.
- Design notes:
  - Architecture and routes are limited to indexer/search/proxy/rate-limit/Torznab needs.
  - Media management endpoints are intentionally excluded in v1.
- Test coverage summary:
  - Documentation-only change; no new tests added.
- Observability updates:
  - No changes; existing telemetry plans remain in effect.
- Risk & rollback plan:
  - Risk: future work bypasses the scope gate. Rollback by reasserting scope in a follow-up
    ADR and pruning out-of-scope routes.
- Dependency rationale:
  - No new dependencies added.
