# Avoid `sqlx-named-bind`

- Status: Accepted
- Date: 2025-11-28

## Context
- We considered adding the `sqlx-named-bind` crate to allow `:name`-style parameters on SQL queries.
- Current policy (ADR-014) centralises SQL in `revaer-data` and requires stored procedures with explicit named arguments (`_arg => $1`), and AGENT.md pushes for minimal dependencies.
- Introducing another proc-macro layer would broaden the attack surface and add coupling to `sqlx`’s internal SQL parsing while providing limited benefit because we already control SQL strings in the DAL.

## Decision
- Do **not** adopt `sqlx-named-bind`. Continue using plain `sqlx` with stored procedure calls and explicit `_arg => $1` named argument mapping in the DAL.

## Consequences
- Keeps the dependency footprint and build complexity unchanged.
- Avoids compatibility and security risks from an additional proc-macro tied to `sqlx` internals.
- Engineers must continue to enforce named-argument stored procedure calls manually in `revaer-data`.

## Follow-up
- None now. If future requirements force raw SQL ergonomics, revisit with a new ADR that justifies the dependency, version pinning, and testing/CI coverage.***
