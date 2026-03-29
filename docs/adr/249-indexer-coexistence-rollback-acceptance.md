# 248: Indexer coexistence and rollback acceptance coverage

- Status: Accepted
- Date: 2026-03-20

## Motivation

- `ERD_INDEXERS.md` requires migration reversibility: Revaer must run alongside Prowlarr, avoid destructive Arr mutations, and keep rollback to a Torznab URL change.
- The repo already had parity/import coverage, but not an explicit acceptance slice proving coexistence and the lack of downstream-app mutation surfaces.

## Design notes

- Added an API E2E spec that creates multiple Revaer Torznab instances, runs import flow activity alongside them, and verifies both endpoints stay callable.
- Added an operator-facing rollback guide that documents the intended migration safety net.
- Guarded the public API surface by asserting the OpenAPI document does not expose downstream Arr mutation routes.

## Test coverage summary

- Added `tests/specs/api/indexers-coexistence-rollback.spec.ts`.
- Covered coexistence of multiple Torznab instances and rollback-safety assertions against the published API surface.

## Observability updates

- No telemetry changes. This slice adds acceptance coverage and operator documentation only.

## Risk & rollback plan

- Risk is low because the implementation adds tests and documentation without changing runtime behavior.
- Roll back by reverting the spec, guide, and checklist/ADR updates if the acceptance framing changes.

## Dependency rationale

- No new dependencies added.
