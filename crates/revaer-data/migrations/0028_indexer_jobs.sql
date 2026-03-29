-- Job scheduling.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'job_key') THEN
        CREATE TYPE job_key AS ENUM (
            'retention_purge',
            'reputation_rollup_1h',
            'reputation_rollup_24h',
            'reputation_rollup_7d',
            'connectivity_profile_refresh',
            'canonical_backfill_best_source',
            'base_score_refresh_recent',
            'canonical_prune_low_confidence',
            'policy_snapshot_gc',
            'policy_snapshot_refcount_repair',
            'rate_limit_state_purge',
            'rss_poll',
            'rss_subscription_backfill'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS job_schedule (
    job_schedule_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    job_key job_key NOT NULL,
    cadence_seconds INTEGER NOT NULL
        CHECK (cadence_seconds BETWEEN 30 AND 604800),
    jitter_seconds INTEGER NOT NULL DEFAULT 0
        CHECK (jitter_seconds >= 0 AND jitter_seconds <= cadence_seconds),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_run_at TIMESTAMPTZ,
    next_run_at TIMESTAMPTZ NOT NULL,
    locked_until TIMESTAMPTZ,
    lock_owner VARCHAR(128),
    CONSTRAINT job_schedule_job_key_uq UNIQUE (job_key)
);
