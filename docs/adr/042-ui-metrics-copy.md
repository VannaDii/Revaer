# 042 - UI Metrics Copy Button (Task Record)

- Status: In Progress
- Date: 2025-12-24

## Motivation
- Provide a fast way to copy `/metrics` output from the Health page.
- Close the optional metrics viewer requirement in the dashboard checklist.

## Design Notes
- Keep clipboard access in `app` to respect the "window-only in app" rule.
- Use a HealthPage callback to avoid side effects in the feature view.
- Emit success/error toasts to confirm copy status.

## Decision
- Use the Clipboard API (`navigator.clipboard.writeText`) for copying.
- Guard the copy button when metrics payload is empty.

## Consequences
- Operators can copy metrics text without leaving the UI.
- Clipboard permissions may block copy; errors are surfaced via toasts.

## Test Coverage Summary
- UI-only change; no new Rust tests added.

## Observability Updates
- None.

## Risk & Rollback
- Risk: clipboard API unavailable in some browsers.
- Rollback: remove the copy button and clipboard helper.

## Dependency Rationale
- Enable the existing `web-sys` Clipboard feature to access `navigator.clipboard`.
- Alternative considered: legacy `execCommand("copy")`, avoided due to deprecation.
