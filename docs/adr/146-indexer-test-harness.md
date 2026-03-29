# 145 Indexer stored-proc test harness

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Indexer stored-proc wrappers have extensive integration tests with repeated DB setup.
  - ERD_INDEXERS_CHECKLIST requires a transactional, seeded harness and deterministic clocks.
  - We need consistent setup without introducing new dependencies.
- Decision:
  - Add a shared `IndexerTestDb` helper in `revaer-data::indexers` (test-only).
  - Centralize Postgres startup, migrations, and UTC session configuration.
  - Capture a deterministic `now()` value after migrations for tests that need time inputs.
- Consequences:
  - Tests share a single harness, reducing setup drift and boilerplate.
  - Deterministic timestamps are available without leaking production code changes.
  - Test-only helper code is now part of the indexer module.
- Follow-up:
  - Use `IndexerTestDb::now()` in additional tests that depend on timestamps.
  - Add explicit transaction helpers if we need per-test rollbacks beyond isolated DBs.

## Motivation

Indexer stored procedures are covered by integration tests that previously duplicated database
startup and migration logic. The checklist calls for a consistent harness with deterministic
clocks and seeded data. A shared helper keeps the setup aligned and makes it easier to maintain.

## Design notes

- Tests use `IndexerTestDb` to keep the disposable database alive for the test duration.
- The helper configures session time zone to UTC and captures a single `now()` value after
  migrations for deterministic timestamp inputs.
- No production code paths or runtime behavior are changed.

## Test coverage summary

- `just ci`
- `just ui-e2e`

## Observability updates

- None.

## Risk & rollback plan

- Risk: tests may rely on helper behavior and need updates if the harness evolves.
- Rollback: revert this ADR and restore per-test setup helpers.

## Dependency rationale

- No new dependencies.
