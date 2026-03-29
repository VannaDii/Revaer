# 251: Indexer final acceptance closeout

- Status: accepted
- Date: 2026-03-21

## Motivation

- `ERD_INDEXERS.md` defines a hard-blocker migration acceptance bar, but the checklist still left `Final acceptance criteria (all hard blockers) pass` unchecked after the underlying API, Torznab, import, and rollback coverage had already landed across multiple slices.
- We needed one explicit closeout step that ties the current evidence back to the ERD's go/no-go criteria so the remaining unchecked items stay limited to non-hard-blocker follow-up work.

## Design notes

- Added `tests/specs/api/indexers-final-acceptance.spec.ts` as a focused acceptance aggregation test.
- The new spec verifies the hard-blocker user path remains:
  - explicit for invalid Torznab queries,
  - explicit for missing downloads,
  - explicit for missing import secrets,
  - reversible with no downstream app mutation surface.
- The checklist is updated to mark final acceptance complete while preserving the still-open non-hard-blocker parity gaps for app sync UX, app-scoped category overrides, broader import UX, and health notifications.

## Test coverage summary

- Added `tests/specs/api/indexers-final-acceptance.spec.ts`.
- Existing supporting coverage remains in:
  - `tests/specs/api/indexers-migration-parity.spec.ts`
  - `tests/specs/api/indexers-import-jobs.spec.ts`
  - `tests/specs/api/indexers-coexistence-rollback.spec.ts`

## Observability updates

- No production observability changes were required.
- Acceptance evidence continues to rely on existing import, Torznab, and rollback endpoint behavior plus the previously shipped health/explainability surfaces.

## Risk & rollback plan

- Risk is low because this change closes an acceptance gap with additive verification and documentation rather than altering runtime behavior.
- If any acceptance assumption regresses, rollback is a straightforward revert of this ADR, the acceptance spec, and the checklist update while keeping the earlier feature slices intact.

## Dependency rationale

- No new dependencies were added.
- Alternative considered: leave final acceptance unchecked until every non-hard-blocker parity item landed. Rejected because the ERD separates hard blockers from follow-up UX parity, and the repo already has the necessary migration-safety evidence to close the hard-blocker gate now.
