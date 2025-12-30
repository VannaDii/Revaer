# Auth Expiry + Error Context Fields

- Status: Accepted
- Date: 2025-12-28
- Context:
  - Factory reset failures must surface raw error details to clients without embedding context in error messages.
  - Setup completion must issue an API key that expires after 14 days, and expiration must be enforced server-side.
  - JSONB-based helpers are disallowed; legacy helpers must be removed while preserving upgrade paths.
- Decision:
  - Add an optional `expires_at` timestamp to `auth_api_keys` and extend API key upsert helpers to persist it.
  - Extend RFC9457 `ProblemDetails` with structured `context` fields so raw error details can be returned separately from constant error messages.
  - Purge JSONB-based helper functions during migration to keep final database surfaces JSON-free.
- Consequences:
  - Positive outcomes: API key expiry is enforced consistently; error responses can include raw details without violating message rules; migrations end with JSONB-free functions.
  - Risks or trade-offs: Existing API clients must tolerate the new `context` field; migrations rely on drop logic to clear legacy helper functions.
- Follow-up:
  - Implementation tasks: update API key auth reads to respect expiry; add error context plumbing in API/UI clients; keep openapi export in sync.
  - Review checkpoints: verify migrations run cleanly, JSONB functions are absent, and factory reset errors surface in toasts.
