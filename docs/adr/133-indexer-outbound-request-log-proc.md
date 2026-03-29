# Indexer outbound request log procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Outbound request telemetry must be written through stored procedures with strict validation and normalized cursor diagnostics.
  - The ERD mandates URL-aware page cursor normalization and hashing to bound storage.
- Decision:
  - Add `outbound_request_log_write_v1` to validate request invariants, resolve public IDs, normalize page cursor keys, persist outbound request logs, and update run correlation tracking.
  - Provide a stable `outbound_request_log_write` wrapper for versioned usage.
- Consequences:
  - Outbound request samples are consistent across callers and safe for rollups.
  - Cursor normalization adds complexity; malformed cursor input now fails fast instead of being stored.
- Follow-up:
  - Wire outbound logging from search runs and indexer probes to use the new procedure.
  - Add DB-level tests for cursor normalization and rate-limit invariants once the data test harness exists.
