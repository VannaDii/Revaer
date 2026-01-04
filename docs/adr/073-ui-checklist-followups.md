# UI checklist follow-ups: SSE detail refresh, labels shortcuts, strict i18n, and anymap removal

- Status: Accepted
- Date: 2026-01-03

## Motivation

- Close remaining dashboard UI checklist gaps tied to live metadata, labels navigation, and strict i18n.
- Remove the vendored yewdux/anymap fork and the related advisory ignore now that upstream versions align. (Superseded by ADR 074 for Yew compatibility.)

## Context

- SSE metadata updates did not refresh list-row tags/tracker/category without a full list refresh.
- Add/Create torrent modals lacked shortcuts into the Settings → Labels workflow.
- Translation fallback masked missing keys; the checklist requires explicit missing-key surfacing.
- `anymap` advisory `RUSTSEC-2021-0065` was previously ignored due to the vendored store fork.

## Decision

- Add a throttled, targeted torrent detail refresh path for metadata events and reuse detail summaries to update list rows.
- Add `on_manage_labels` callbacks in torrent modals to route directly to the Labels tab.
- Remove i18n fallback behavior and add explicit English copy for new UI affordances.
- Dependency alignment is superseded by ADR 074 (vendored yewdux for Yew 0.22 compatibility).
- Drop the advisory ignore tied to the vendored `anymap`.
- Remove remaining vendored crates (`hashlink`, `sqlx-core`) and rely on registry sources.

## Design Notes

- Use a debounced HashSet queue to coalesce detail refreshes and avoid duplicate fetches.
- Settings accepts a `requested_tab` prop and clears it once the tab selection is applied.
- Translation bundles return `missing:{key}` for missing entries; no default locale fallback.
- `upsert_detail` updates list-row tags, tracker, category, and name/path using the detail summary.

## Consequences

- Tags/trackers/categories update without full list refreshes, reducing UI staleness.
- Users can reach label management quickly from torrent modals.
- Missing translations are obvious during QA instead of silently falling back.
- Supply-chain ignores shrink with the removal of vendored `anymap`.
- Dependency alignment outcomes are tracked in ADR 074.

## Test Coverage Summary

- `just ci`: blocked by `just cov` (workspace line coverage 76.46%).
- `just cov`: fails `--fail-under-lines 80` (TOTAL line coverage 76.46%).

## Observability Updates

- None.

## Risk & Rollback Plan

- Risk: targeted refreshes could increase detail fetch volume under heavy metadata churn.
- Rollback: revert the targeted refresh scheduler and restore the prior full refresh behavior.

## Dependency Rationale

- Dependency alignment decisions moved to ADR 074 to capture the vendored yewdux exception.

## Follow-up

- Verify labels shortcuts and SSE metadata refresh during QA.
