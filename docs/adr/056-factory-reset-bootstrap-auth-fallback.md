# Factory reset bootstrap auth fallback

- Status: Accepted
- Date: 2025-12-28
- Context:
  - Factory reset requires API key auth, but existing installs can be in `active` mode with zero API keys (pre-bootstrap).
  - Without a key, the UI cannot authenticate and the system has no recovery path.
  - The reset path must still use stored procedures and surface raw errors when the reset fails.
- Decision:
  - Add a `has_api_keys` capability to the config facade so the API can detect empty key inventories.
  - Introduce a factory-reset-specific auth gate that accepts valid API keys, or allows the reset when no API keys exist (logging a warning).
  - Keep confirmation phrase validation unchanged.
- Consequences:
  - Provides a recovery path for deployments missing API keys.
  - When no API keys exist, factory reset can be triggered without auth; this is acceptable because the system is already unauthenticated in that state.
- Follow-up:
  - Consider tightening the fallback to loopback-only requests if new auth modes are added.
  - Ensure UI messaging continues to surface authorization errors via toasts.
