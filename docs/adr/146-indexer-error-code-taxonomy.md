# Indexer error-code taxonomy

- Status: Accepted
- Date: 2026-01-27
- Context:
  - Stored procedures already raise exceptions with `DETAIL` codes, but there is no single,
    documented taxonomy for the values or how the API must surface them.
  - AGENTS.md requires constant error messages, structured context fields, and stable
    error mapping for clients and tests.
- Decision:
  - Define a shared error-code taxonomy for indexer stored procedures and API responses:
    - Stored procedures:
      - Domain/validation/authorization failures raise `ERRCODE = 'P0001'` with a constant
        `MESSAGE` of the form `Failed to <operation>` and `DETAIL` set to the error code.
      - Infrastructure/constraint errors use native SQLSTATE codes (e.g., `23505`, `23503`)
        and do not override the Postgres message.
      - `DETAIL` values are lower_snake_case, <= 64 chars, and never embed user data.
    - API responses:
      - Use RFC9457 Problem responses with constant `title`/`detail` strings.
      - Include `error_code` (from the DB `DETAIL`) and `sqlstate` as `context` fields when
        present, never interpolated into human-readable messages.
      - Validation errors prefer `invalid_params` with constant messages; contextual inputs
        travel in `context` fields.
  - Adopt the following canonical error-code groups (examples are non-exhaustive):
    - Missing/empty/length: `*_missing`, `*_empty`, `*_too_long`, `*_too_short`.
    - Format/normalization: `*_not_lowercase`, `*_invalid_format`, `*_invalid`.
    - Lookup/identity: `*_not_found`, `*_reference_missing`, `unknown_key`.
    - Conflicts/state: `*_already_exists`, `*_deleted`, `*_in_use`, `*_disabled`.
    - Unsupported/blocked: `unsupported_*`, `*_disallowed`, `*_insufficient`.
    - Auth/actor: `actor_missing`, `actor_not_found`, `actor_unauthorized`.
- Consequences:
  - Clients can reliably map failures by `error_code` while keeping UI text constant and
    localizable.
  - Tests can assert stable `error_code`/`sqlstate` values without parsing messages.
- Follow-up:
  - Enforce taxonomy compliance in new stored procedures and API handlers.
  - Extend integration tests to cover new error codes as endpoints are added.

## Task record

- Motivation:
  - Provide a single, stable taxonomy for indexer errors so DB, API, CLI, and UI agree on
    machine-readable codes while keeping messages constant.
- Design notes:
  - DB procs keep `MESSAGE` constant and carry machine codes in `DETAIL`.
  - API handlers surface `error_code`/`sqlstate` via `ProblemDetails.context` and keep
    `detail` text constant for localization.
- Test coverage summary:
  - Documentation-only change; no new tests added.
- Observability updates:
  - Errors continue to log with structured fields (`error_code`, `sqlstate`) at the origin.
- Risk & rollback plan:
  - Risk: taxonomy drift if future procs introduce ad-hoc codes. Rollback by reverting this
    ADR and aligning new procedures to existing ad-hoc behavior.
- Dependency rationale:
  - No new dependencies added.
