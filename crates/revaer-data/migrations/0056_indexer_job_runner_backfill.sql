-- Backfill jobs for canonical best source and RSS subscriptions.

CREATE OR REPLACE FUNCTION job_run_canonical_backfill_best_source_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    cutoff_recent TIMESTAMPTZ;
    canonical_public_id UUID;
BEGIN
    cutoff_recent := now() - make_interval(days => 7);

    FOR canonical_public_id IN
        SELECT c.canonical_torrent_public_id
        FROM canonical_torrent c
        LEFT JOIN canonical_torrent_best_source_global b
            ON b.canonical_torrent_id = c.canonical_torrent_id
        WHERE b.canonical_torrent_id IS NULL
           OR c.created_at >= cutoff_recent
           OR (c.identity_strategy = 'title_size_fallback' AND c.identity_confidence <= 0.60)
    LOOP
        PERFORM canonical_recompute_best_source_v1(canonical_public_id, 'global_current');
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION job_run_canonical_backfill_best_source()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_canonical_backfill_best_source_v1();
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rss_subscription_backfill_v1()
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    maintenance_completed_at TIMESTAMPTZ;
BEGIN
    SELECT rss_subscription_backfill_completed_at
    INTO maintenance_completed_at
    FROM deployment_maintenance_state
    ORDER BY deployment_maintenance_state_id
    LIMIT 1;

    IF maintenance_completed_at IS NOT NULL THEN
        UPDATE job_schedule
        SET enabled = FALSE
        WHERE job_key = 'rss_subscription_backfill';
        RETURN;
    END IF;

    INSERT INTO indexer_rss_subscription (
        indexer_instance_id,
        is_enabled,
        interval_seconds,
        last_polled_at,
        next_poll_at,
        backoff_seconds,
        last_error_class
    )
    SELECT inst.indexer_instance_id,
           (inst.is_enabled AND inst.enable_rss),
           900,
           NULL,
           CASE
               WHEN inst.is_enabled AND inst.enable_rss THEN
                   now() + make_interval(secs => floor(random() * 60)::INT)
               ELSE NULL
           END,
           NULL,
           NULL
    FROM indexer_instance inst
    LEFT JOIN indexer_rss_subscription sub
        ON sub.indexer_instance_id = inst.indexer_instance_id
    WHERE sub.indexer_instance_id IS NULL;

    UPDATE deployment_maintenance_state
    SET rss_subscription_backfill_completed_at = now(),
        last_updated_at = now();

    IF NOT FOUND THEN
        INSERT INTO deployment_maintenance_state (
            rss_subscription_backfill_completed_at,
            last_updated_at
        )
        VALUES (
            now(),
            now()
        );
    END IF;

    UPDATE job_schedule
    SET enabled = FALSE
    WHERE job_key = 'rss_subscription_backfill';
END;
$$;

CREATE OR REPLACE FUNCTION job_run_rss_subscription_backfill()
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM job_run_rss_subscription_backfill_v1();
END;
$$;
