# UI Dashboard Nexus Parity Tweaks

- Status: Accepted
- Date: 2025-12-27
- Context:
  - Dashboard cards drifted from the vendored Nexus markup and referenced missing i18n keys.
  - Connectivity modal included fields outside the required SSE status spec.
  - Constraints: keep Nexus layout structure, use DaisyUI semantic tokens, avoid new dependencies.
- Decision:
  - Rework the storage usage and tracker health cards to match Nexus layout structure and available translation keys.
  - Align queue summary/global summary labels to existing nav/dashboard strings.
  - Trim the SSE connectivity modal to the required fields and labels.
  - Replace dashboard recent events table markup with a DaisyUI list layout.
  - Limit SSE indicator label expansion to the sidebar expanded state only.
  - Alternatives considered: adding new translation keys across all locales (rejected for scope and translation burden).
- Consequences:
  - Positive outcomes: fewer missing strings, closer Nexus parity, clearer SSE status display.
  - Risks or trade-offs: storage usage detail reduced to summary metrics; some labels remain static in English where Nexus requires them.
- Follow-up:
  - Manually verify Nexus dashboard parity and table hover styling in the UI.

## Motivation
- Restore Nexus layout parity for dashboard sections and eliminate missing dashboard translation keys.

## Design Notes
- Storage usage mirrors the Nexus revenue card layout with the chart slot preserved.
- Tracker health metrics follow the Nexus acquisition grid with two columns and error count in the header.
- Queue summary and global summary labels use existing nav/dashboard translations.
- SSE connectivity modal aligns with the required status fields only.
- Recent events use a DaisyUI list layout that preserves the Nexus header structure.
- Row-hover styling applies to list rows for parity with table hover behavior.

## Test Coverage Summary
- No new tests added; UI-only changes.

## Observability Updates
- None.

## Risk & Rollback Plan
- Low risk; revert the UI component edits if layout regressions appear.

## Dependency Rationale
- No new dependencies introduced.
