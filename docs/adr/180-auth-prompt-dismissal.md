# Auth prompt dismissal stability

- Status: Accepted
- Date: 2026-02-01
- Context:
  - Motivation: UI E2E intermittently fails because the auth prompt reappears after dismissal when
    app auth mode is resolved asynchronously.
  - Constraints: Preserve current auth behavior, avoid new dependencies, keep state logic testable.
- Decision:
  - Stop resetting `auth_prompt_dismissed` in the app auth mode effect so a user dismissal remains
    effective for the session.
  - Alternatives considered: re-trying dismissal in tests only, or persisting dismissal in storage.
  - Dependency rationale: none (state-only change).
- Consequences:
  - Positive outcomes: auth overlay no longer reappears after dismissal during initial config
    hydration; UI tests can dismiss overlays reliably.
  - Risks or trade-offs: users might need to re-open auth prompt manually if they dismissed it while
    auth becomes required; rollback by reintroducing reset with a timestamp or explicit user action.
- Follow-up:
  - Implementation tasks: adjust app auth mode effect to avoid overriding dismissal state.
  - Test coverage summary: UI E2E coverage exercises overlay dismissal; no new unit tests added.
  - Observability updates: none.
