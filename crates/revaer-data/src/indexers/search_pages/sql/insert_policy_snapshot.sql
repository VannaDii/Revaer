INSERT INTO policy_snapshot (snapshot_hash, ref_count)
VALUES ($1, $2)
RETURNING policy_snapshot_id
