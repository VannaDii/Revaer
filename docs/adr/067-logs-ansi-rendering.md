# Logs ANSI rendering and bounded buffer

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: logs view must preserve ANSI color/style codes, Unicode characters, and remain responsive over long sessions.
  - Constraints: keep memory usage bounded, avoid new dependencies, keep layout aligned with UI rules, and avoid build conflicts with `trunk serve`.
- Decision:
  - Parse ANSI SGR sequences into styled spans for rendering with theme-aware colors.
  - Keep a bounded in-memory log buffer with a fixed max size.
  - Use streaming text decode to preserve multibyte characters across chunks.
  - Add new log lines to the top of the view and restrict scrolling to the terminal area.
  - Use a dedicated `dist-serve` directory for `trunk serve` to avoid staging conflicts with `ui-build`.
- Consequences:
  - Positive outcomes: log output retains color/style and Unicode, memory growth is capped, log background is black.
  - Risks or trade-offs: ANSI color mapping approximates terminal colors via theme tokens and CSS variables.
- Follow-up:
  - Implementation tasks: monitor logs stream for any unhandled ANSI sequences and extend parsing as needed.
  - Review checkpoints: run `just ci` before handoff.

## Test coverage summary
- `just ui-build`: failed (wasm-bindgen could not write to staging directory while `trunk serve` was running).

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: unusual ANSI sequences may render as plain text.
- Rollback: remove ANSI parsing and revert to raw log line rendering.
