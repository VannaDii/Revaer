-- Disambiguate search_page_fetch_v1 column references.

CREATE OR REPLACE FUNCTION search_page_fetch_v1(
    actor_user_public_id UUID,
    search_request_public_id_input UUID,
    page_number_input INTEGER
)
RETURNS TABLE(
    page_number INTEGER,
    sealed_at TIMESTAMPTZ,
    item_count INTEGER,
    item_position INTEGER,
    canonical_torrent_public_id UUID,
    title_display VARCHAR,
    size_bytes BIGINT,
    infohash_v1 CHAR(40),
    infohash_v2 CHAR(64),
    magnet_hash CHAR(64),
    canonical_torrent_source_public_id UUID,
    indexer_instance_public_id UUID,
    indexer_display_name VARCHAR,
    seeders INTEGER,
    leechers INTEGER,
    published_at TIMESTAMPTZ,
    download_url VARCHAR,
    magnet_uri VARCHAR,
    details_url VARCHAR,
    tracker_name VARCHAR,
    tracker_category INTEGER,
    tracker_subcategory INTEGER
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to fetch search page';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    request_id BIGINT;
    request_user_id BIGINT;
    page_id BIGINT;
    sealed_at_value TIMESTAMPTZ;
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

    IF search_request_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_missing';
    END IF;

    SELECT search_request_id, user_id
    INTO request_id, request_user_id
    FROM search_request
    WHERE search_request_public_id = search_request_public_id_input;

    IF request_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_request_not_found';
    END IF;

    IF request_user_id IS NULL THEN
        IF actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    ELSE
        IF request_user_id <> actor_user_id AND actor_role NOT IN ('owner', 'admin') THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'actor_unauthorized';
        END IF;
    END IF;

    IF page_number_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'page_number_missing';
    END IF;

    IF page_number_input < 1 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'page_number_invalid';
    END IF;

    SELECT sp.search_page_id, sp.sealed_at
    INTO page_id, sealed_at_value
    FROM search_page AS sp
    WHERE sp.search_request_id = request_id
      AND sp.page_number = page_number_input;

    IF page_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'search_page_not_found';
    END IF;

    RETURN QUERY
    SELECT
        page_number_input AS page_number,
        sealed_at_value AS sealed_at,
        COALESCE(item_counts.item_count, 0) AS item_count,
        spi.position AS item_position,
        ct.canonical_torrent_public_id,
        ct.title_display,
        COALESCE(ct.size_bytes, cts.size_bytes) AS size_bytes,
        ct.infohash_v1,
        ct.infohash_v2,
        ct.magnet_hash,
        cts.canonical_torrent_source_public_id,
        ii.indexer_instance_public_id,
        ii.display_name,
        cts.last_seen_seeders,
        cts.last_seen_leechers,
        cts.last_seen_published_at,
        cts.last_seen_download_url,
        cts.last_seen_magnet_uri,
        cts.last_seen_details_url,
        tracker_name.value_text,
        tracker_category.value_int,
        tracker_subcategory.value_int
    FROM (SELECT 1) AS seed
    LEFT JOIN LATERAL (
        SELECT COUNT(*)::INTEGER AS item_count
        FROM search_page_item
        WHERE search_page_id = page_id
    ) AS item_counts ON TRUE
    LEFT JOIN search_page_item spi
        ON spi.search_page_id = page_id
    LEFT JOIN search_request_canonical src
        ON src.search_request_canonical_id = spi.search_request_canonical_id
    LEFT JOIN canonical_torrent ct
        ON ct.canonical_torrent_id = src.canonical_torrent_id
    LEFT JOIN canonical_torrent_best_source_context bs
        ON bs.context_key_type = 'search_request'
        AND bs.context_key_id = request_id
        AND bs.canonical_torrent_id = ct.canonical_torrent_id
    LEFT JOIN canonical_torrent_source cts
        ON cts.canonical_torrent_source_id = bs.canonical_torrent_source_id
    LEFT JOIN indexer_instance ii
        ON ii.indexer_instance_id = cts.indexer_instance_id
    LEFT JOIN LATERAL (
        SELECT value_text
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = cts.canonical_torrent_source_id
          AND attr_key = 'tracker_name'
    ) AS tracker_name ON TRUE
    LEFT JOIN LATERAL (
        SELECT value_int
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = cts.canonical_torrent_source_id
          AND attr_key = 'tracker_category'
    ) AS tracker_category ON TRUE
    LEFT JOIN LATERAL (
        SELECT value_int
        FROM canonical_torrent_source_attr
        WHERE canonical_torrent_source_id = cts.canonical_torrent_source_id
          AND attr_key = 'tracker_subcategory'
    ) AS tracker_subcategory ON TRUE
    ORDER BY spi.position;
END;
$$;
