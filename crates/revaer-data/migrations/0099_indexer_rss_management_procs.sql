-- RSS management read/write procedures for operator tooling.

CREATE OR REPLACE FUNCTION indexer_rss_subscription_get_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    indexer_instance_public_id UUID,
    instance_is_enabled BOOLEAN,
    instance_enable_rss BOOLEAN,
    subscription_exists BOOLEAN,
    subscription_is_enabled BOOLEAN,
    interval_seconds INTEGER,
    last_polled_at TIMESTAMPTZ,
    next_poll_at TIMESTAMPTZ,
    backoff_seconds INTEGER,
    last_error_class error_class
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch RSS subscription';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_deleted';
    END IF;

    RETURN QUERY
    SELECT
        inst.indexer_instance_public_id,
        inst.is_enabled,
        inst.enable_rss,
        sub.indexer_rss_subscription_id IS NOT NULL,
        COALESCE(sub.is_enabled, FALSE),
        COALESCE(sub.interval_seconds, 900),
        sub.last_polled_at,
        sub.next_poll_at,
        sub.backoff_seconds,
        sub.last_error_class
    FROM indexer_instance inst
    LEFT JOIN indexer_rss_subscription sub
        ON sub.indexer_instance_id = inst.indexer_instance_id
    WHERE inst.indexer_instance_id = instance_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_item_seen_list_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    limit_input INTEGER
)
RETURNS TABLE (
    item_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    first_seen_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to list RSS items';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    item_limit INTEGER;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_deleted';
    END IF;

    item_limit := COALESCE(limit_input, 25);
    IF item_limit < 1 OR item_limit > 200 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'limit_out_of_range';
    END IF;

    RETURN QUERY
    SELECT
        seen.item_guid,
        seen.infohash_v1,
        seen.infohash_v2,
        seen.magnet_hash,
        seen.first_seen_at
    FROM indexer_rss_item_seen seen
    WHERE seen.indexer_instance_id = instance_id
    ORDER BY seen.first_seen_at DESC, seen.rss_item_seen_id DESC
    LIMIT item_limit;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_item_seen_mark_v1(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    item_guid_input VARCHAR,
    infohash_v1_input CHAR(40),
    infohash_v2_input CHAR(64),
    magnet_hash_input CHAR(64)
)
RETURNS TABLE (
    item_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    first_seen_at TIMESTAMPTZ,
    inserted BOOLEAN
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to mark RSS item seen';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    item_guid_value VARCHAR(256);
    infohash_v1_value CHAR(40);
    infohash_v2_value CHAR(64);
    magnet_hash_value CHAR(64);
    first_seen_value TIMESTAMPTZ;
    inserted_value BOOLEAN := FALSE;
BEGIN
    IF actor_user_public_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_missing';
    END IF;

    SELECT user_id, role
    INTO actor_user_id, actor_role
    FROM app_user
    WHERE user_public_id = actor_user_public_id;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_not_found';
    END IF;

    IF actor_role NOT IN ('owner', 'admin') THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'actor_unauthorized';
    END IF;

    IF indexer_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_missing';
    END IF;

    SELECT indexer_instance_id, deleted_at
    INTO instance_id, instance_deleted_at
    FROM indexer_instance
    WHERE indexer_instance_public_id = indexer_instance_public_id_input;

    IF instance_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'indexer_deleted';
    END IF;

    item_guid_value := NULLIF(lower(trim(item_guid_input)), '');
    infohash_v1_value := NULLIF(lower(trim(infohash_v1_input)), '')::CHAR(40);
    infohash_v2_value := NULLIF(lower(trim(infohash_v2_input)), '')::CHAR(64);
    magnet_hash_value := NULLIF(lower(trim(magnet_hash_input)), '')::CHAR(64);

    IF item_guid_value IS NOT NULL AND char_length(item_guid_value) > 256 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'item_guid_too_long';
    END IF;

    IF infohash_v1_value IS NOT NULL AND infohash_v1_value !~ '^[0-9a-f]{40}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'infohash_v1_invalid';
    END IF;

    IF infohash_v2_value IS NOT NULL AND infohash_v2_value !~ '^[0-9a-f]{64}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'infohash_v2_invalid';
    END IF;

    IF magnet_hash_value IS NOT NULL AND magnet_hash_value !~ '^[0-9a-f]{64}$' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'magnet_hash_invalid';
    END IF;

    IF item_guid_value IS NULL
        AND infohash_v1_value IS NULL
        AND infohash_v2_value IS NULL
        AND magnet_hash_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'rss_item_identifier_missing';
    END IF;

    INSERT INTO indexer_rss_item_seen (
        indexer_instance_id,
        item_guid,
        infohash_v1,
        infohash_v2,
        magnet_hash,
        first_seen_at
    )
    VALUES (
        instance_id,
        item_guid_value,
        infohash_v1_value,
        infohash_v2_value,
        magnet_hash_value,
        now()
    )
    ON CONFLICT DO NOTHING
    RETURNING indexer_rss_item_seen.first_seen_at
    INTO first_seen_value;

    inserted_value := first_seen_value IS NOT NULL;

    IF inserted_value IS FALSE THEN
        SELECT seen.first_seen_at
        INTO first_seen_value
        FROM indexer_rss_item_seen seen
        WHERE seen.indexer_instance_id = instance_id
          AND (
              (item_guid_value IS NOT NULL AND seen.item_guid = item_guid_value)
              OR (infohash_v1_value IS NOT NULL AND seen.infohash_v1 = infohash_v1_value)
              OR (infohash_v2_value IS NOT NULL AND seen.infohash_v2 = infohash_v2_value)
              OR (magnet_hash_value IS NOT NULL AND seen.magnet_hash = magnet_hash_value)
          )
        ORDER BY seen.first_seen_at DESC, seen.rss_item_seen_id DESC
        LIMIT 1;
    END IF;

    RETURN QUERY
    SELECT
        item_guid_value,
        infohash_v1_value,
        infohash_v2_value,
        magnet_hash_value,
        first_seen_value,
        inserted_value;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_subscription_get(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID
)
RETURNS TABLE (
    indexer_instance_public_id UUID,
    instance_is_enabled BOOLEAN,
    instance_enable_rss BOOLEAN,
    subscription_exists BOOLEAN,
    subscription_is_enabled BOOLEAN,
    interval_seconds INTEGER,
    last_polled_at TIMESTAMPTZ,
    next_poll_at TIMESTAMPTZ,
    backoff_seconds INTEGER,
    last_error_class error_class
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_rss_subscription_get_v1(
        actor_user_public_id => actor_user_public_id,
        indexer_instance_public_id_input => indexer_instance_public_id_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_item_seen_list(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    limit_input INTEGER
)
RETURNS TABLE (
    item_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    first_seen_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_rss_item_seen_list_v1(
        actor_user_public_id => actor_user_public_id,
        indexer_instance_public_id_input => indexer_instance_public_id_input,
        limit_input => limit_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_rss_item_seen_mark(
    actor_user_public_id UUID,
    indexer_instance_public_id_input UUID,
    item_guid_input VARCHAR,
    infohash_v1_input CHAR(40),
    infohash_v2_input CHAR(64),
    magnet_hash_input CHAR(64)
)
RETURNS TABLE (
    item_guid VARCHAR(256),
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    first_seen_at TIMESTAMPTZ,
    inserted BOOLEAN
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_rss_item_seen_mark_v1(
        actor_user_public_id => actor_user_public_id,
        indexer_instance_public_id_input => indexer_instance_public_id_input,
        item_guid_input => item_guid_input,
        infohash_v1_input => infohash_v1_input,
        infohash_v2_input => infohash_v2_input,
        magnet_hash_input => magnet_hash_input
    );
END;
$$;
