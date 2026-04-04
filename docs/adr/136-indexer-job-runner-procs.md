# Indexer job runner procedures

- Status: Accepted
- Date: 2026-01-26
- Context:
  - Background job scheduling requires database-enforced claiming and retention cleanup.
  - Retention rules must align with deployment_config thresholds and avoid deleting durable data.
- Decision:
  - Add `job_claim_next_v1` to enforce lease-based claiming with advisory locks and per-job lease durations.
  - Add `job_run_retention_purge_v1` to purge completed search trees and operational telemetry using retention thresholds.
- Consequences:
  - Job claiming is serialized per job_key and prevents overlapping workers.
  - Retention cleanup reduces operational data growth while preserving durable records.
- Follow-up:
  - Add per-job completion procedures that advance next_run_at with jitter and clear locks.
  - Add test coverage for retention purge edge cases once the data test harness exists.
