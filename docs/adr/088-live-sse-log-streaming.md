# Live SSE Log Streaming

- Status: Accepted
- Date: 2026-01-17
- Context:
  - Motivation: remove dummy SSE data, ensure SSE is the single live update channel, and surface recent logs immediately on open.
  - Constraints: keep the log stream lightweight, avoid new dependencies, and respect existing SSE routes.
- Decision:
  - Summary: drop the dummy SSE stream, retain a rolling two-minute log buffer for SSE snapshots, and add log level filtering + text search in the UI.
  - Design notes: telemetry now snapshots recent log lines, the API chains the snapshot ahead of the live broadcast, and UI log lines track level + receipt time for filtering and pruning.
  - Dependency rationale: no new dependencies; reuse existing serde_json parsing in the UI for log level detection.
- Consequences:
  - Positive outcomes: SSE reflects live event data only, logs open with context, and the logs page can filter by level or search text.
  - Risks or trade-offs: some log lines may skip buffer storage under contention, and non-drop SSE errors now require manual retry instead of automatic reconnect.
  - Risk & rollback plan: revert the log buffer/snapshot changes to restore streaming-only behavior and re-enable auto-reconnect if needed.
- Follow-up:
  - Implementation tasks: adjust telemetry buffering, SSE handlers, and logs UI controls with filtering/search state.
  - Test coverage summary: added log buffer tests; run `just ci` and `just ui-e2e` to validate full coverage.
  - Observability updates: log stream now captures a rolling snapshot; SSE status remains visible via existing UI badges.
