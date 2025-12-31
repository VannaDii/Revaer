# Factory reset UX fallback and SSE setup gating

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: SSE returns 409 when the server is in setup mode, leaving the UI stuck after factory reset or manual setup transitions.
  - Constraints: keep the UI non-blocking, avoid API key reuse after reset, and keep state transitions client-driven without new dependencies.
- Decision:
  - Gate SSE connection on `AppModeState` and surface a disconnected status when the server is in setup mode.
  - Treat SSE 409 responses as a setup signal: clear auth state and move the app into setup mode in the store.
  - Ensure factory reset success forces `AppModeState::Setup` even if the reload fails.
- Consequences:
  - Positive outcomes: factory reset lands users on the setup flow; SSE no longer loops on 409 responses.
  - Risks or trade-offs: clears stored auth on setup transitions, requiring re-auth after reset.
- Follow-up:
  - Implementation tasks: monitor setup flows for any unexpected auth clears and adjust messaging if needed.
  - Review checkpoints: run `just ci` and `just build-release` before handoff.

## Design notes
- SSE is disabled in setup mode to prevent repeated 409 retries and to keep the UI responsive.
- Setup transitions clear auth storage to avoid stale API keys after reset.

## Test coverage summary
- `just ci`: failed (`cargo llvm-cov` line coverage 77.59% < 80%).

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: users expecting to keep API keys across resets will have to re-authenticate.
- Rollback: remove SSE setup gating and 409 handling, revert factory reset UI state updates, and restore previous auth persistence behavior.
