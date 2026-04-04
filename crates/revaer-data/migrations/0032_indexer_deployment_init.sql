-- Deployment initialization stored procedures.

CREATE OR REPLACE FUNCTION deployment_init_v1(actor_user_public_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to initialize deployment';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    actor_verified BOOLEAN;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role, is_email_verified
    INTO actor_user_id, actor_role, actor_verified
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_verified IS DISTINCT FROM TRUE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unverified';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    INSERT INTO app_user (
        user_id,
        user_public_id,
        email,
        email_normalized,
        is_email_verified,
        display_name,
        role,
        created_at
    ) OVERRIDING SYSTEM VALUE
    SELECT
        0,
        '00000000-0000-0000-0000-000000000000',
        'system@revaer.local',
        'system@revaer.local',
        TRUE,
        'System',
        'owner',
        now()
    WHERE NOT EXISTS (
        SELECT 1
        FROM app_user
        WHERE user_id = 0
           OR user_public_id = '00000000-0000-0000-0000-000000000000'
    );

    PERFORM trust_tier_seed_defaults();
    PERFORM media_domain_seed_defaults();

    IF NOT EXISTS (SELECT 1 FROM deployment_config) THEN
        INSERT INTO deployment_config DEFAULT VALUES;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM deployment_maintenance_state) THEN
        INSERT INTO deployment_maintenance_state DEFAULT VALUES;
    END IF;

    INSERT INTO rate_limit_policy (
        rate_limit_policy_public_id,
        display_name,
        requests_per_minute,
        burst,
        concurrent_requests,
        is_system
    )
    VALUES
        (gen_random_uuid(), 'default_indexer', 60, 30, 2, TRUE),
        (gen_random_uuid(), 'default_routing', 120, 60, 4, TRUE)
    ON CONFLICT (display_name) DO NOTHING;

    WITH seed_jobs (job_key, cadence_seconds, enabled) AS (
        VALUES
            ('retention_purge'::job_key, 3600, TRUE),
            ('reputation_rollup_1h'::job_key, 300, TRUE),
            ('reputation_rollup_24h'::job_key, 3600, TRUE),
            ('reputation_rollup_7d'::job_key, 21600, TRUE),
            ('connectivity_profile_refresh'::job_key, 300, TRUE),
            ('canonical_backfill_best_source'::job_key, 86400, TRUE),
            ('base_score_refresh_recent'::job_key, 3600, TRUE),
            ('canonical_prune_low_confidence'::job_key, 86400, TRUE),
            ('policy_snapshot_gc'::job_key, 86400, TRUE),
            ('policy_snapshot_refcount_repair'::job_key, 86400, TRUE),
            ('rate_limit_state_purge'::job_key, 3600, TRUE),
            ('rss_poll'::job_key, 60, TRUE),
            ('rss_subscription_backfill'::job_key, 300, TRUE)
    )
    INSERT INTO job_schedule (
        job_key,
        cadence_seconds,
        jitter_seconds,
        enabled,
        next_run_at
    )
    SELECT
        seed_jobs.job_key,
        seed_jobs.cadence_seconds,
        0,
        seed_jobs.enabled,
        now() + make_interval(
            secs => random_jitter_seconds(seed_jobs.cadence_seconds - 1)
        )
    FROM seed_jobs
    ON CONFLICT (job_key) DO NOTHING;
END;
$$;

CREATE OR REPLACE FUNCTION deployment_init(actor_user_public_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM deployment_init_v1(actor_user_public_id);
END;
$$;
