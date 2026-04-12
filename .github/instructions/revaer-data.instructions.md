---
applyTo:
  - "crates/revaer-data/**"
  - "crates/**/migrations/**"
  - "scripts/dev-seed.sql"
---

`AGENTS.md` and `rust.instructions.md` apply first. This file specializes database and migration work.

# Database Rules

- Runtime application code must call stored procedures for database behavior. Do not embed inline business SQL in Rust.
- `just lint` mechanically enforces that `sqlx::query*` usage stays confined to `crates/revaer-data/src`, and that inline DDL/DML text does not appear in authored Rust.
- Raw DDL, DML, stored procedure bodies, and seed SQL belong in migrations or tightly scoped operational bootstrap scripts only.
- `JSONB` and related conglomerate persistence formats are banned for application state.
- Shared behavior lives in shared stored procedures. Do not duplicate the same database behavior across multiple crates.
- Use named bind parameters and explicit transactions where a multi-step change must be atomic.

# Migration Rules

- Every behavior change that affects persisted state ships with a migration.
- Migrations must be versioned, deterministic, and safe to replay in clean environments.
- If the runtime behavior changes, update the stored procedure layer and the Rust caller in the same change.

# Testing

- Exercise database behavior through the same stored procedure entry points that production uses.
- Keep migration and procedure tests representative of runtime call patterns.
- If a migration or procedure change affects API or CLI behavior, update the relevant docs and task record in the same change.
