# 144 Indexer domain mapping and DI boundaries

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Indexer work spans stored procedures, API surfaces, UI/CLI usage, and background jobs.
  - ERD_INDEXERS.md requires clear domain boundaries and injected dependencies.
  - Testability and stored-proc-only data access must stay consistent across crates.
- Decision:
  - Map indexer domains to existing crates and define DI seams for each domain service.
  - Versioned stored procedures use `_v1` suffixes with stable wrapper functions without version suffixes.
- Consequences:
  - Clear ownership reduces cross-crate coupling and supports isolated testing.
  - API/UI/CLI can share a single facade surface without leaking database details.
  - Procedure evolution can continue without breaking callers by updating wrappers.
- Follow-up:
  - Implement per-domain facades in `revaer-api` and wire concrete implementations in `revaer-app`.
  - Add tests per facade and for stored-proc wrappers to enforce error-style consistency.

## Domain-to-crate mapping

- `revaer-data`:
  - Stored-proc wrappers and result mapping for indexer domains under `crates/revaer-data/src/indexers/*`.
  - Error types scoped to data access with constant messages and structured context.
- `revaer-api`:
  - HTTP handlers under `crates/revaer-api/src/http/handlers/indexers/*`.
  - Domain facades and traits under `crates/revaer-api/src/app/indexers/*` (API-safe DTOs only).
- `revaer-app`:
  - Bootstrap wiring in `crates/revaer-app/src/bootstrap.rs` for concrete data-layer implementations.
- `revaer-cli`:
  - CLI commands call API endpoints only; no direct data access.
- `revaer-ui`:
  - UI uses `services/*` and feature slices; no direct data access.
- `revaer-events` / `revaer-telemetry`:
  - Event publication and metrics for indexer operations at the API boundary.

## DI boundaries (facade surface)

Expose API-facing traits in `revaer-api::app::indexers` and inject concrete implementations from
`revaer-app`:

- `IndexerDefinitionsService`: definitions catalog and field metadata.
- `IndexerInstancesService`: create/update instances, RSS settings, field values, tag/media-domain binds.
- `RoutingPolicyService`: create/update policies, params, and secrets.
- `SecretsService`: create/rotate/revoke/read secrets and bindings.
- `TagsService`: create/update/delete tags.
- `SearchProfilesService`: profiles, trust tiers, domain/tag filters, and policy-set wiring.
- `PoliciesService`: policy sets/rules management and snapshot refresh hooks.
- `TorznabService`: torznab instance lifecycle and category mappings.
- `ImportsService`: import job lifecycle and status reporting.
- `JobsService`: job claim/run entry points for indexer background jobs.
- `CanonicalizationService`: canonical maintenance and disambiguation rules.
- `ReputationService`: connectivity and reputation rollups.

All facades return `Result<T, E>` with constant error messages and structured context fields.
No facade constructs concrete dependencies; all implementations are injected from bootstrap.

## Motivation

Document and lock the indexer architecture mapping needed to implement the ERD without leaking
database details or violating dependency-injection rules.

## Design notes

- Reuse existing crates/modules; avoid introducing new crates until feature growth demands it.
- Keep stored-proc wrappers in `revaer-data` and expose only API-safe DTOs at the HTTP boundary.

## Test coverage summary

- `just ci`
- `just ui-e2e`

## Observability updates

- None.

## Risk & rollback plan

- Risk: documentation drift if code moves without updating this ADR.
- Rollback: revert this ADR and restore checklist items to unchecked.

## Dependency rationale

- No new dependencies.
