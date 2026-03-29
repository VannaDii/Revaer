# 249. Indexer Domain Service Closeout

Date: 2026-03-20

## Status

Accepted

## Context

- The ERD checklist still carried the phase-6 domain-service item even though the current app-layer indexer service already fronts the shipped indexer domains.
- That stale unchecked item obscured the real remaining gaps, which are product-facing features like app sync, category overrides, richer import UX, and health notification delivery.

## Decision

- Close the phase-6 domain-service checklist item after auditing the existing service boundary.
- Treat `crates/revaer-app/src/indexers.rs` as the application-service boundary for the shipped indexer surface:
  - catalog and definition reads
  - tags and secrets
  - search orchestration reads and writes
  - routing policies and rate-limit policies
  - search profiles and tracker category mappings
  - import jobs and backup/restore flows
  - Torznab access, indexer instance lifecycle, RSS, and connectivity/reputation reads
- Treat the runtime/data modules as the implementation site for the non-CRUD execution domains named by the checklist:
  - policy evaluation
  - canonicalization and conflict handling
  - reputation/connectivity rollups
  - background job execution

## Consequences

- The checklist now reflects the actual architecture instead of implying a missing service layer.
- The remaining unchecked ERD items stay focused on user-visible gaps that still need code, schema, and UX work.

## Task Record

Motivation:
- Remove a stale incomplete marker once the service-layer audit confirmed the phase-6 work is already implemented.

Design notes:
- Audited `IndexerService` in `crates/revaer-app/src/indexers.rs` against the checklist language and existing runtime/data modules.
- Kept the dependency-injection boundary unchanged: bootstrap constructs concrete services, while the app layer exposes injected indexer operations.

Test coverage summary:
- No new runtime path was introduced.
- Existing `just ci` and `just ui-e2e` continue to cover the already-shipped service surface.

Observability updates:
- No new telemetry changes were required; the existing service layer already emits `indexer.*` spans and metrics.

Risk & rollback plan:
- Low risk because this is a checklist and ADR closeout for already-shipped code.
- Roll back by restoring the checklist item to unchecked if a later audit finds a missing domain-service boundary.

Dependency rationale:
- No dependency changes.
