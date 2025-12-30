# 014 – Centralized Data Access Layer

-   Status: Accepted
-   Date: 2025-02-14

## Context

-   We’ve historically embedded SQL across `revaer-config`, `revaer-fsops`, and runtime-oriented crates, which made behavioral auditing and policy changes slow.
-   AGENT.md now mandates that **all runtime SQL lives in stored procedures** with named parameter bindings, and migrations must be a single flat sequence to avoid drift.
-   We also need a single place to share Postgres helpers (migrations, Testcontainers harness, schema structs) so that coverage and policy changes don’t require touching every crate.

## Decision

-   Introduce a dedicated `revaer-data` crate that owns:
    -   Migration assets for config + runtime schemas in a single baseline migration (`crates/revaer-data/migrations/0007_rebaseline.sql`).
    -   Stored procedures in the `revaer_config` schema that wrap every CRUD/query operation (history, revision bumps, setup tokens, secrets, API keys, config profiles, fs/engine/app mutations).
    -   Rust helpers (`crates/revaer-data/src/config.rs` and `runtime.rs`) that only ever call those stored procedures using named bind notation.
-   Consumers (config service, fsops tests, orchestrator runtime store, etc.) depend on `revaer-data` instead of embedding SQL. Integration tests that previously queried tables directly now call the DAL API.
-   Migrations are consolidated into a single init script so that initial setup is deterministic without managing multiple numbered files.

## Consequences

-   **Positive**
    -   One migration stream and schema owner simplifies rollout/rollback and satisfies the “flat list” rule.
    -   Stored procedure coverage is explicit; adding a new DB touch point requires updating `revaer-data` and its migrations, so AGENT compliance is easier to enforce.
    -   Integration tests gained better fidelity by exercising the same code paths used in production; no more `sqlx::query` literals outside the DAL.
-   **Trade-offs**
    -   Any schema change now requires touching `revaer-data` plus the stored procedure definitions, which adds upfront work.
    -   Consumers must depend on `revaer-data` even for simple read paths; we have to watch for accidental circular deps.

## Follow-up

-   Keep adding stored procedures as new DB operations emerge; the DAL is now the only sanctioned place for SQL.
-   Automate ADR publishing (`mdBook`) once `just docs` picks up the new entry.
-   Enforce the `revaer-data` dependency in lint (e.g., deny `sqlx::query` outside the crate) to prevent regressions.\*\*\* End Patch
