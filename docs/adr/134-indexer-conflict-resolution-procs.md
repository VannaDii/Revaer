# Indexer conflict resolution procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Source metadata conflicts require operator resolution with strict authorization and audit logging.
  - Accepted-incoming resolutions must never overwrite existing durable data.
- Decision:
  - Add `source_metadata_conflict_resolve_v1` and `source_metadata_conflict_reopen_v1` to enforce admin/owner authorization, apply limited backfills, and record audit events.
  - Limit accepted-incoming updates to safe backfills (source_guid, tracker_name, tracker_category/subcategory) when the durable value is missing.
- Consequences:
  - Conflict resolution is traceable and safe against overwrites.
  - Incoming tracker category parsing is validated; malformed inputs are rejected instead of silently stored.
- Follow-up:
  - Add test coverage for conflict resolution paths once the data test harness exists.
