-- Ensure the first page-visible source is recorded as the search-request best source.

CREATE OR REPLACE FUNCTION search_result_ingest(
    search_request_public_id_input UUID,
    indexer_instance_public_id_input UUID,
    source_guid_input VARCHAR,
    details_url_input VARCHAR,
    download_url_input VARCHAR,
    magnet_uri_input VARCHAR,
    title_raw_input VARCHAR,
    size_bytes_input BIGINT,
    infohash_v1_input CHAR(40),
    infohash_v2_input CHAR(64),
    magnet_hash_input CHAR(64),
    seeders_input INTEGER,
    leechers_input INTEGER,
    published_at_input TIMESTAMPTZ,
    uploader_input VARCHAR,
    observed_at_input TIMESTAMPTZ,
    attr_keys_input observation_attr_key[],
    attr_types_input attr_value_type[],
    attr_value_text_input VARCHAR[],
    attr_value_int_input INTEGER[],
    attr_value_bigint_input BIGINT[],
    attr_value_numeric_input NUMERIC(12, 4)[],
    attr_value_bool_input BOOLEAN[],
    attr_value_uuid_input UUID[]
)
RETURNS TABLE(
    canonical_torrent_public_id UUID,
    canonical_torrent_source_public_id UUID,
    observation_created BOOLEAN,
    durable_source_created BOOLEAN,
    canonical_changed BOOLEAN
)
LANGUAGE plpgsql
AS $$
DECLARE
    request_id_value BIGINT;
    canonical_id_value BIGINT;
    source_id_value BIGINT;
    canonical_public_id_value UUID;
    canonical_source_public_id_value UUID;
BEGIN
    SELECT *
    INTO
        canonical_public_id_value,
        canonical_source_public_id_value,
        observation_created,
        durable_source_created,
        canonical_changed
    FROM search_result_ingest_v1(
        search_request_public_id_input,
        indexer_instance_public_id_input,
        source_guid_input,
        details_url_input,
        download_url_input,
        magnet_uri_input,
        title_raw_input,
        size_bytes_input,
        infohash_v1_input,
        infohash_v2_input,
        magnet_hash_input,
        seeders_input,
        leechers_input,
        published_at_input,
        uploader_input,
        observed_at_input,
        attr_keys_input,
        attr_types_input,
        attr_value_text_input,
        attr_value_int_input,
        attr_value_bigint_input,
        attr_value_numeric_input,
        attr_value_bool_input,
        attr_value_uuid_input
    );

    canonical_torrent_public_id := canonical_public_id_value;
    canonical_torrent_source_public_id := canonical_source_public_id_value;

    IF canonical_public_id_value IS NOT NULL
        AND canonical_source_public_id_value IS NOT NULL THEN
        SELECT search_request_id
        INTO request_id_value
        FROM search_request
        WHERE search_request_public_id = search_request_public_id_input;

        SELECT canonical_torrent_id
        INTO canonical_id_value
        FROM canonical_torrent
        WHERE canonical_torrent.canonical_torrent_public_id = canonical_public_id_value;

        SELECT canonical_torrent_source_id
        INTO source_id_value
        FROM canonical_torrent_source
        WHERE canonical_torrent_source.canonical_torrent_source_public_id =
            canonical_source_public_id_value;

        IF request_id_value IS NOT NULL
            AND canonical_id_value IS NOT NULL
            AND source_id_value IS NOT NULL
            AND EXISTS (
                SELECT 1
                FROM search_request_canonical src
                JOIN search_page_item spi
                    ON spi.search_request_canonical_id = src.search_request_canonical_id
                WHERE src.search_request_id = request_id_value
                  AND src.canonical_torrent_id = canonical_id_value
            ) THEN
            INSERT INTO canonical_torrent_best_source_context (
                context_key_type,
                context_key_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                computed_at
            )
            VALUES (
                'search_request',
                request_id_value,
                canonical_id_value,
                source_id_value,
                now()
            )
            ON CONFLICT (context_key_type, context_key_id, canonical_torrent_id)
            DO UPDATE SET
                canonical_torrent_source_id = EXCLUDED.canonical_torrent_source_id,
                computed_at = EXCLUDED.computed_at;
        END IF;
    END IF;

    RETURN NEXT;
END;
$$;
