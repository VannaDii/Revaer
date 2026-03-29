# Indexer Tag API Surface

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Indexer tag stored procedures exist but there is no HTTP surface or service wiring.
  - The API layer needs a DI-friendly facade to keep handlers thin and testable.
  - Errors must use constant messages with structured context fields.
- Decision:
  - Introduce an indexer facade trait in `revaer-api` and implement it in `revaer-app`.
  - Add `/v1/indexers/tags` create/update/delete endpoints using stored procedures.
  - Publish tag DTOs in `revaer-api-models` and update OpenAPI.
- Consequences:
  - API callers can manage indexer tags without direct database access.
  - API server construction now requires an indexer facade dependency.
  - Tests and wiring must supply a stub indexer implementation.
- Follow-up:
  - Extend indexer API coverage for definitions, instances, routing, secrets, and policies.
  - Add list/read endpoints once read procedures are defined.

## Motivation

Provide a clean, testable HTTP surface for indexer tag management that aligns with the ERD and
stored-procedure contract.

## Design notes

- The API layer delegates to a narrow `IndexerFacade` trait to keep handlers minimal.
- Tag operations pass the system actor UUID while user identity is not yet plumbed.
- Service errors carry error codes and SQLSTATE without interpolating values into messages.

## Test coverage summary

- Added handler tests for tag create and error mapping (bad request/not found).
- Existing API tests updated to supply a stub indexer facade.

## Observability updates

- Indexer service logs storage/authorization failures with structured fields (`operation`,
  `error_code`, `sqlstate`).

## Risk & rollback plan

- Risk: new routes expose tag mutations before full RBAC is enforced.
- Rollback: revert the tag handler/routes and facade wiring commits.

## Dependency rationale

- No new dependencies added; existing `revaer-api`, `revaer-app`, and data-layer crates are reused.
