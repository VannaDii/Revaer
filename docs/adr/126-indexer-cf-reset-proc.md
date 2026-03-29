# Indexer Cloudflare reset procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Operators need a controlled reset path for Cloudflare challenges and cooldowns.
  - ERD_INDEXERS.md requires owner/admin authorization, CF state reset, and conditional
    connectivity profile recovery for quarantined indexers.
- Decision:
  - Add migration 0036 with indexer_cf_state_reset_v1 plus a stable wrapper.
  - Reset cf_state to clear, wipe CF session/cooldown/backoff metadata, and zero
    consecutive_failures.
  - If connectivity status is quarantined with CF-related error classes, downgrade to
    degraded and clear error_class to unknown.
  - Record a config_audit_log update with change_summary "cf_state reset".
- Consequences:
  - CF recovery can be triggered safely with auditable changes.
  - Non-CF connectivity failures are preserved.
- Follow-up:
  - Add API handler wiring for CF resets.
  - Add tests for quarantined vs non-quarantined connectivity transitions.
