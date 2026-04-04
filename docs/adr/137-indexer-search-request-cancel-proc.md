# Indexer search request cancel procedure

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Search requests must be cancelable with proper authorization and clean terminal state transitions.
  - Runs in queued or running state must be marked canceled without violating status timestamp constraints.
- Decision:
  - Add `search_request_cancel_v1` to enforce actor authorization, mark the search as canceled, and cancel in-flight runs.
  - Keep the procedure idempotent when the request is already terminal.
- Consequences:
  - Cancel operations consistently update finished_at/canceled_at and avoid invalid run states.
  - Unauthorized callers cannot cancel Torznab-owned searches.
- Follow-up:
  - Implement `search_request_create_v1` and search run state procedures to complete the search lifecycle.
