-- Fix wrapper argument binding ambiguity for RSS management procs.

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
        actor_user_public_id,
        indexer_instance_public_id_input
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
        actor_user_public_id,
        indexer_instance_public_id_input,
        limit_input
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
        actor_user_public_id,
        indexer_instance_public_id_input,
        item_guid_input,
        infohash_v1_input,
        infohash_v2_input,
        magnet_hash_input
    );
END;
$$;
