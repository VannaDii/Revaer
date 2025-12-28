# Factory Reset and Bootstrap API Key

- Status: Accepted
- Date: 2025-12-27
- Context:
  - Need a safe factory reset workflow that keeps navigation available while enforcing confirmation.
  - Setup completion must return a bootstrap API key with a 14-day client-side expiry.
  - Raw reset errors must surface to the UI for operator visibility.
- Decision:
  - Add `revaer_config.factory_reset()` stored procedure and `/admin/factory-reset` API endpoint guarded by API key auth.
  - Ensure setup completion provisions or reuses a bootstrap API key and returns it with an expiry timestamp.
  - Persist the bootstrap API key with expiry in local storage and require manual dismissal for error toasts.
- Consequences:
  - Factory reset clears configuration/runtime data and returns the system to setup mode.
  - API key expiry is enforced on the client; the server remains stateless about expiry.
  - Reset failures are delivered verbatim to clients for display.
- Follow-up:
  - Update OpenAPI export, UI dropdown + modal wiring, and storage helpers.
  - Verify CI and runtime migrations.
