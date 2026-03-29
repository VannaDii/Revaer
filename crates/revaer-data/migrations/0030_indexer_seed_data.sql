-- Seed defaults for trust tiers, media domains, torznab categories, and job scheduling.

CREATE OR REPLACE FUNCTION random_jitter_seconds(max_seconds INTEGER)
RETURNS INTEGER
LANGUAGE plpgsql
AS $$
DECLARE
    errcode CONSTANT text := 'P0001';
    bytes BYTEA;
    value BIGINT;
BEGIN
    IF max_seconds < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = 'Failed to compute random jitter',
            DETAIL = 'max_seconds_negative';
    END IF;

    bytes := gen_random_bytes(4);
    value := (get_byte(bytes, 0)::BIGINT << 24)
        + (get_byte(bytes, 1)::BIGINT << 16)
        + (get_byte(bytes, 2)::BIGINT << 8)
        + get_byte(bytes, 3)::BIGINT;

    RETURN (value % (max_seconds + 1))::INTEGER;
END;
$$;

CREATE OR REPLACE FUNCTION trust_tier_seed_defaults()
RETURNS VOID
LANGUAGE plpgsql
AS $$DECLARE
    base_message CONSTANT text := 'Failed to seed trust tiers';
    errcode CONSTANT text := 'P0001';

BEGIN
    INSERT INTO trust_tier (trust_tier_key, display_name, default_weight, rank)
    VALUES
        ('public', 'Public', 0, 10),
        ('semi_private', 'Semi-Private', 5, 20),
        ('private', 'Private', 10, 30),
        ('invite_only', 'Invite Only', 15, 40)
    ON CONFLICT (trust_tier_key) DO NOTHING;

    IF EXISTS (
        SELECT 1
        FROM trust_tier
        WHERE trust_tier_key = 'public'
          AND (
              display_name IS DISTINCT FROM 'Public'
              OR default_weight IS DISTINCT FROM 0
              OR rank IS DISTINCT FROM 10
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM trust_tier
        WHERE trust_tier_key = 'semi_private'
          AND (
              display_name IS DISTINCT FROM 'Semi-Private'
              OR default_weight IS DISTINCT FROM 5
              OR rank IS DISTINCT FROM 20
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM trust_tier
        WHERE trust_tier_key = 'private'
          AND (
              display_name IS DISTINCT FROM 'Private'
              OR default_weight IS DISTINCT FROM 10
              OR rank IS DISTINCT FROM 30
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM trust_tier
        WHERE trust_tier_key = 'invite_only'
          AND (
              display_name IS DISTINCT FROM 'Invite Only'
              OR default_weight IS DISTINCT FROM 15
              OR rank IS DISTINCT FROM 40
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION media_domain_seed_defaults()
RETURNS VOID
LANGUAGE plpgsql
AS $$DECLARE
    errcode CONSTANT text := 'P0001';

BEGIN
    INSERT INTO media_domain (media_domain_key, display_name)
    VALUES
        ('movies', 'Movies'),
        ('tv', 'TV'),
        ('audiobooks', 'Audiobooks'),
        ('ebooks', 'Ebooks'),
        ('software', 'Software'),
        ('adult_movies', 'Adult Movies'),
        ('adult_scenes', 'Adult Scenes')
    ON CONFLICT (media_domain_key) DO NOTHING;

    IF EXISTS (
        SELECT 1
        FROM media_domain
        WHERE media_domain_key::TEXT <> lower(media_domain_key::TEXT)
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = 'Failed to seed media domains',
            DETAIL = 'media_domain_key_not_lowercase';
    END IF;
END;
$$;

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

SELECT trust_tier_seed_defaults();
SELECT media_domain_seed_defaults();

WITH seed_categories (torznab_cat_id, name) AS (
    VALUES
        (2000, 'Movies'),
        (2010, 'Movies/2010'),
        (2020, 'Movies/2020'),
        (2030, 'Movies/2030'),
        (2040, 'Movies/2040'),
        (2045, 'Movies/2045'),
        (2050, 'Movies/2050'),
        (2060, 'Movies/2060'),
        (3000, 'Audio'),
        (3010, 'Audio/3010'),
        (3020, 'Audio/3020'),
        (4000, 'Software'),
        (4050, 'Software/4050'),
        (5000, 'TV'),
        (5010, 'TV/5010'),
        (5020, 'TV/5020'),
        (5030, 'TV/5030'),
        (5040, 'TV/5040'),
        (5045, 'TV/5045'),
        (5050, 'TV/5050'),
        (5060, 'TV/5060'),
        (5070, 'TV/5070'),
        (5075, 'TV/5075'),
        (5080, 'TV/5080'),
        (6000, 'Adult'),
        (6010, 'Adult/6010'),
        (6020, 'Adult/6020'),
        (6030, 'Adult/6030'),
        (6040, 'Adult/6040'),
        (7000, 'Books'),
        (7010, 'Books/7010'),
        (7020, 'Books/7020'),
        (8000, 'Other')
)
INSERT INTO torznab_category (torznab_cat_id, name)
SELECT torznab_cat_id, name
FROM seed_categories
ON CONFLICT (torznab_cat_id) DO NOTHING;

WITH seed_mapping (media_domain_key, torznab_cat_id, is_primary) AS (
    VALUES
        ('movies'::media_domain_key, 2000, TRUE),
        ('movies'::media_domain_key, 2010, FALSE),
        ('movies'::media_domain_key, 2020, FALSE),
        ('movies'::media_domain_key, 2030, FALSE),
        ('movies'::media_domain_key, 2040, FALSE),
        ('movies'::media_domain_key, 2045, FALSE),
        ('movies'::media_domain_key, 2050, FALSE),
        ('movies'::media_domain_key, 2060, FALSE),
        ('tv'::media_domain_key, 5000, TRUE),
        ('tv'::media_domain_key, 5010, FALSE),
        ('tv'::media_domain_key, 5020, FALSE),
        ('tv'::media_domain_key, 5030, FALSE),
        ('tv'::media_domain_key, 5040, FALSE),
        ('tv'::media_domain_key, 5045, FALSE),
        ('tv'::media_domain_key, 5050, FALSE),
        ('tv'::media_domain_key, 5060, FALSE),
        ('tv'::media_domain_key, 5070, FALSE),
        ('tv'::media_domain_key, 5075, FALSE),
        ('tv'::media_domain_key, 5080, FALSE),
        ('audiobooks'::media_domain_key, 3020, TRUE),
        ('ebooks'::media_domain_key, 7000, FALSE),
        ('ebooks'::media_domain_key, 7010, TRUE),
        ('ebooks'::media_domain_key, 7020, FALSE),
        ('software'::media_domain_key, 4000, TRUE),
        ('software'::media_domain_key, 4050, FALSE),
        ('adult_movies'::media_domain_key, 6000, TRUE),
        ('adult_movies'::media_domain_key, 6010, FALSE),
        ('adult_movies'::media_domain_key, 6020, FALSE),
        ('adult_movies'::media_domain_key, 6030, FALSE),
        ('adult_movies'::media_domain_key, 6040, FALSE),
        ('adult_scenes'::media_domain_key, 6000, TRUE),
        ('adult_scenes'::media_domain_key, 6010, FALSE),
        ('adult_scenes'::media_domain_key, 6020, FALSE),
        ('adult_scenes'::media_domain_key, 6030, FALSE),
        ('adult_scenes'::media_domain_key, 6040, FALSE)
)
INSERT INTO media_domain_to_torznab_category (
    media_domain_id,
    torznab_category_id,
    is_primary
)
SELECT
    media_domain.media_domain_id,
    torznab_category.torznab_category_id,
    seed_mapping.is_primary
FROM seed_mapping
JOIN media_domain
    ON media_domain.media_domain_key = seed_mapping.media_domain_key
JOIN torznab_category
    ON torznab_category.torznab_cat_id = seed_mapping.torznab_cat_id
ON CONFLICT (media_domain_id, torznab_category_id) DO UPDATE
    SET is_primary = EXCLUDED.is_primary;

WITH seed_tracker_mapping (tracker_category, torznab_cat_id, media_domain_key) AS (
    VALUES
        (2000, 2000, 'movies'::media_domain_key),
        (2010, 2010, 'movies'::media_domain_key),
        (2020, 2020, 'movies'::media_domain_key),
        (2030, 2030, 'movies'::media_domain_key),
        (2040, 2040, 'movies'::media_domain_key),
        (2045, 2045, 'movies'::media_domain_key),
        (2050, 2050, 'movies'::media_domain_key),
        (2060, 2060, 'movies'::media_domain_key),
        (5000, 5000, 'tv'::media_domain_key),
        (5010, 5010, 'tv'::media_domain_key),
        (5020, 5020, 'tv'::media_domain_key),
        (5030, 5030, 'tv'::media_domain_key),
        (5040, 5040, 'tv'::media_domain_key),
        (5045, 5045, 'tv'::media_domain_key),
        (5050, 5050, 'tv'::media_domain_key),
        (5060, 5060, 'tv'::media_domain_key),
        (5070, 5070, 'tv'::media_domain_key),
        (5075, 5075, 'tv'::media_domain_key),
        (5080, 5080, 'tv'::media_domain_key),
        (7000, 7000, 'ebooks'::media_domain_key),
        (7010, 7010, 'ebooks'::media_domain_key),
        (7020, 7020, 'ebooks'::media_domain_key),
        (3020, 3020, 'audiobooks'::media_domain_key),
        (4000, 4000, 'software'::media_domain_key),
        (4050, 4050, 'software'::media_domain_key),
        (6000, 6000, 'adult_movies'::media_domain_key),
        (6010, 6010, 'adult_movies'::media_domain_key),
        (6020, 6020, 'adult_movies'::media_domain_key),
        (6030, 6030, 'adult_movies'::media_domain_key),
        (6040, 6040, 'adult_movies'::media_domain_key),
        (3000, 3000, NULL::media_domain_key),
        (3010, 3010, NULL::media_domain_key),
        (8000, 8000, NULL::media_domain_key)
)
INSERT INTO tracker_category_mapping (
    indexer_definition_id,
    tracker_category,
    tracker_subcategory,
    torznab_category_id,
    media_domain_id
)
SELECT
    NULL,
    seed_tracker_mapping.tracker_category,
    0,
    torznab_category.torznab_category_id,
    media_domain.media_domain_id
FROM seed_tracker_mapping
JOIN torznab_category
    ON torznab_category.torznab_cat_id = seed_tracker_mapping.torznab_cat_id
LEFT JOIN media_domain
    ON media_domain.media_domain_key = seed_tracker_mapping.media_domain_key
WHERE NOT EXISTS (
    SELECT 1
    FROM tracker_category_mapping existing
    WHERE existing.indexer_definition_id IS NULL
      AND existing.tracker_category = seed_tracker_mapping.tracker_category
      AND existing.tracker_subcategory = 0
);

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

DO $$
DECLARE
    base_message CONSTANT text := 'Failed to seed rate limit policies';
    errcode CONSTANT text := 'P0001';
BEGIN
    IF EXISTS (
        SELECT 1
        FROM rate_limit_policy
        WHERE display_name = 'default_indexer'
          AND (
              requests_per_minute IS DISTINCT FROM 60
              OR burst IS DISTINCT FROM 30
              OR concurrent_requests IS DISTINCT FROM 2
              OR is_system IS DISTINCT FROM TRUE
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM rate_limit_policy
        WHERE display_name = 'default_routing'
          AND (
              requests_per_minute IS DISTINCT FROM 120
              OR burst IS DISTINCT FROM 60
              OR concurrent_requests IS DISTINCT FROM 4
              OR is_system IS DISTINCT FROM TRUE
          )
    ) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'seed_values_mismatch';
    END IF;
END;
$$;

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
