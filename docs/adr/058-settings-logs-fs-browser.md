# UI Settings Controls, Logs Stream, and Filesystem Browser

- Status: Accepted
- Date: 2025-12-28
- Context:
  - Motivation: replace JSON settings editing with structured controls, add an on-demand logs view, and provide a server-backed filesystem browser for path selection.
  - Constraints: keep stored-procedure access, avoid new dependencies, and only stream logs while the Logs route is active.
- Decision:
  - Added an SSE logs stream backed by a log broadcast writer and a Logs UI route that connects only while mounted.
  - Added a filesystem browse endpoint and path picker UI for directory selection, with server-side path validation for label policy download dirs.
  - Reworked settings into tabbed sections with a single draft/save bar and structured field editors.
- Consequences:
  - Positive: consistent UI controls, safer path selection, and live logs available without background streaming.
  - Risks: invalid paths now fail validation; recovery requires clearing the offending field or updating the path.
- Follow-up:
  - Tests: no new dependencies; validation logic exercised via existing config pathways (add focused tests if coverage drops).
  - Observability: log stream events emit via SSE; status surfaced in UI badge.
  - Risk & rollback: revert the logs route/endpoint and path validation if regressions appear; keep previous settings UI behind a feature branch.
  - Dependency rationale: no new dependencies added.
