# API key refresh and no-auth setup mode

- Status: Accepted
- Date: 2025-12-30
- Context:
  - Motivation: keep API keys valid without manual re-auth, and allow local setup flows to opt into anonymous access.
  - Constraints: no new dependencies, stored-procedure-only config writes, and API errors localized through i18n.
- Decision:
  - Add `app_profile.auth_mode` with `api_key`/`none` and allow anonymous auth when `none` is configured.
  - Introduce `/v1/auth/refresh` to extend API key expiry without rotation, and schedule refresh in the UI before expiry.
  - Persist anonymous auth state for no-auth setups and reuse the well-known snapshot for setup changeset construction.
  - Store API key expirations in local storage and refresh with a 24-hour safety skew.
- Consequences:
  - Positive outcomes: no-auth local deployments work without API keys; API keys remain valid without user action.
  - Risks or trade-offs: no-auth mode reduces access control if enabled unintentionally; refresh scheduling depends on client time.
- Follow-up:
  - Implementation tasks: keep OpenAPI spec and UI translations in sync with new auth/refresh UX.
  - Review checkpoints: run `just ci` and `just build-release` before handoff.

## Design notes
- Auth mode is stored in `app_profile` and enforced in API auth middleware.
- Token refresh extends expiry only; no rotation or secret re-issuance.

## Test coverage summary
- `just ci`: line coverage 80.03%.
- `just build-release`: succeeded.

## Observability updates
- None.

## Dependency rationale
- No new dependencies added.

## Risk & rollback plan
- Risk: anonymous access enabled on non-local deployments; refresh timing sensitive to client clock drift.
- Rollback: remove `auth_mode`, revert auth middleware and refresh endpoint, and delete UI refresh scheduling plus setup auth mode selection.
