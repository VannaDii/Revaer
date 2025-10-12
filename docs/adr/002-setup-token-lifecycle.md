# 002 – Setup Token Lifecycle & Secrets Bootstrap

- Status: Proposed
- Date: 2025-02-23

## Context
- Initial deployments must boot in a locked-down "Setup Mode" where only a one-time token grants access to the setup API.
- Tokens should be observable/auditable, expire automatically, and support regeneration without requiring an application restart.
- A follow-on requirement is to collect an encryption passphrase or server-side key for pgcrypto-backed secrets before exiting Setup Mode.

## Decision
- Store tokens in the `setup_tokens` table with `token_hash`, `issued_at`, `expires_at`, `consumed_at`, and `issued_by`.
- Enforce at most one active token via a partial unique index on rows where `consumed_at IS NULL`.
- `ConfigService` will:
  - Generate tokens using cryptographically secure randomness.
  - Persist only a hashed representation (argon2id) along with metadata.
  - Emit history entries and `NOTIFY` events on token creation/consumption.
- The CLI/API surfaces token issuance and completion flows; the process prints the token to stdout only at generation time.
- During completion, the caller must supply the encryption materials (passphrase or reference to pgcrypto role). The handler verifies secrets are persisted before flipping `app_profile.mode` to `active`.

## Consequences
- Operators can recover by issuing a new token if the previous one expires without restarting the service.
- Tokens are auditable; failed attempts can be recorded against the hashed token id (future enhancement).
- The bootstrap path ensures secrets exist before runtime modules that require them start, preventing a partially configured system.

## Follow-up
- Implement argon2id hashing helpers and audit logging in `revaer-config`.
- Define the CLI workflow (`revaer-cli setup`) that wraps token issuance and completion for headless environments.
- Add problem detail responses for expired/consumed tokens in the API.
