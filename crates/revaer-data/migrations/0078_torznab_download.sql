-- Torznab download redirect and acquisition attempt logging.

CREATE OR REPLACE FUNCTION torznab_download_prepare_v1(
    torznab_instance_public_id_input UUID,
    canonical_torrent_source_public_id_input UUID
)
RETURNS TABLE(
    redirect_url VARCHAR(2048)
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to prepare torznab download';
    errcode CONSTANT text := 'P0001';
    detail_source_not_in_profile CONSTANT text := 'source_not_in_profile';
    instance_id_value BIGINT;
    profile_id_value BIGINT;
    instance_enabled BOOLEAN;
    instance_deleted_at TIMESTAMPTZ;
    source_id_value BIGINT;
    source_instance_id BIGINT;
    magnet_uri_value VARCHAR(2048);
    download_url_value VARCHAR(2048);
    source_infohash_v1 CHAR(40);
    source_infohash_v2 CHAR(64);
    source_magnet_hash CHAR(64);
    canonical_id_value BIGINT;
    request_id_value BIGINT;
    canonical_infohash_v1 CHAR(40);
    canonical_infohash_v2 CHAR(64);
    canonical_magnet_hash CHAR(64);
    final_infohash_v1 CHAR(40);
    final_infohash_v2 CHAR(64);
    final_magnet_hash CHAR(64);
    allowlist_exists BOOLEAN;
    tag_allowlist_exists BOOLEAN;
    in_allowlist BOOLEAN;
    in_blocklist BOOLEAN;
    in_tag_allowlist BOOLEAN;
    in_tag_blocklist BOOLEAN;
BEGIN
    IF torznab_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_missing';
    END IF;

    IF canonical_torrent_source_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'canonical_source_missing';
    END IF;

    SELECT instance.torznab_instance_id,
           instance.search_profile_id,
           instance.is_enabled,
           instance.deleted_at
    INTO instance_id_value,
         profile_id_value,
         instance_enabled,
         instance_deleted_at
    FROM torznab_instance AS instance
    WHERE instance.torznab_instance_public_id = torznab_instance_public_id_input;

    IF instance_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_not_found';
    END IF;

    IF instance_deleted_at IS NOT NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_deleted';
    END IF;

    IF instance_enabled = FALSE THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_disabled';
    END IF;

    SELECT source.canonical_torrent_source_id,
           source.indexer_instance_id,
           source.last_seen_magnet_uri,
           source.last_seen_download_url,
           source.infohash_v1,
           source.infohash_v2,
           source.magnet_hash
    INTO source_id_value,
         source_instance_id,
         magnet_uri_value,
         download_url_value,
         source_infohash_v1,
         source_infohash_v2,
         source_magnet_hash
    FROM canonical_torrent_source AS source
    WHERE source.canonical_torrent_source_public_id = canonical_torrent_source_public_id_input;

    IF source_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'canonical_source_not_found';
    END IF;

    SELECT EXISTS(
        SELECT 1 FROM search_profile_indexer_allow
        WHERE search_profile_id = profile_id_value
    )
    INTO allowlist_exists;

    IF allowlist_exists THEN
        SELECT EXISTS(
            SELECT 1 FROM search_profile_indexer_allow
            WHERE search_profile_id = profile_id_value
              AND indexer_instance_id = source_instance_id
        )
        INTO in_allowlist;

        IF NOT in_allowlist THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = detail_source_not_in_profile;
        END IF;
    END IF;

    SELECT EXISTS(
        SELECT 1 FROM search_profile_indexer_block
        WHERE search_profile_id = profile_id_value
          AND indexer_instance_id = source_instance_id
    )
    INTO in_blocklist;

    IF in_blocklist THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = detail_source_not_in_profile;
    END IF;

    SELECT EXISTS(
        SELECT 1 FROM search_profile_tag_allow
        WHERE search_profile_id = profile_id_value
    )
    INTO tag_allowlist_exists;

    IF tag_allowlist_exists THEN
        SELECT EXISTS(
            SELECT 1 FROM search_profile_tag_allow sta
            JOIN indexer_instance_tag it
                ON it.tag_id = sta.tag_id
            WHERE sta.search_profile_id = profile_id_value
              AND it.indexer_instance_id = source_instance_id
        )
        INTO in_tag_allowlist;

        IF NOT in_tag_allowlist THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = detail_source_not_in_profile;
        END IF;
    END IF;

    SELECT EXISTS(
        SELECT 1 FROM search_profile_tag_block stb
        JOIN indexer_instance_tag it
            ON it.tag_id = stb.tag_id
        WHERE stb.search_profile_id = profile_id_value
          AND it.indexer_instance_id = source_instance_id
    )
    INTO in_tag_blocklist;

    IF in_tag_blocklist THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = detail_source_not_in_profile;
    END IF;

    SELECT observation.canonical_torrent_id,
           observation.search_request_id
    INTO canonical_id_value,
         request_id_value
    FROM search_request_source_observation AS observation
    WHERE observation.canonical_torrent_source_id = source_id_value
      AND observation.canonical_torrent_id IS NOT NULL
    ORDER BY observation.observed_at DESC
    LIMIT 1;

    IF canonical_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'canonical_not_found';
    END IF;

    SELECT canonical.infohash_v1,
           canonical.infohash_v2,
           canonical.magnet_hash
    INTO canonical_infohash_v1,
         canonical_infohash_v2,
         canonical_magnet_hash
    FROM canonical_torrent AS canonical
    WHERE canonical.canonical_torrent_id = canonical_id_value;

    final_infohash_v1 := COALESCE(canonical_infohash_v1, source_infohash_v1);
    final_infohash_v2 := COALESCE(canonical_infohash_v2, source_infohash_v2);
    final_magnet_hash := COALESCE(canonical_magnet_hash, source_magnet_hash);

    IF final_infohash_v1 IS NULL AND final_infohash_v2 IS NULL AND final_magnet_hash IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'source_missing_hash';
    END IF;

    magnet_uri_value := NULLIF(trim(magnet_uri_value), '');
    download_url_value := NULLIF(trim(download_url_value), '');

    IF magnet_uri_value IS NOT NULL THEN
        redirect_url := magnet_uri_value;
    ELSIF download_url_value IS NOT NULL THEN
        redirect_url := download_url_value;
    ELSE
        redirect_url := NULL;
    END IF;

    IF redirect_url IS NULL THEN
        INSERT INTO acquisition_attempt (
            torznab_instance_id,
            origin,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            user_id,
            infohash_v1,
            infohash_v2,
            magnet_hash,
            torrent_client_name,
            started_at,
            finished_at,
            status,
            failure_class,
            failure_detail
        )
        VALUES (
            instance_id_value,
            'torznab',
            canonical_id_value,
            source_id_value,
            request_id_value,
            NULL,
            final_infohash_v1,
            final_infohash_v2,
            final_magnet_hash,
            'unknown',
            now(),
            now(),
            'failed',
            'client_error',
            'no_download_target'
        );
    ELSE
        INSERT INTO acquisition_attempt (
            torznab_instance_id,
            origin,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            user_id,
            infohash_v1,
            infohash_v2,
            magnet_hash,
            torrent_client_name,
            started_at,
            status
        )
        VALUES (
            instance_id_value,
            'torznab',
            canonical_id_value,
            source_id_value,
            request_id_value,
            NULL,
            final_infohash_v1,
            final_infohash_v2,
            final_magnet_hash,
            'unknown',
            now(),
            'started'
        );
    END IF;

    RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION torznab_download_prepare(
    torznab_instance_public_id_input UUID,
    canonical_torrent_source_public_id_input UUID
)
RETURNS TABLE(
    redirect_url VARCHAR(2048)
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM torznab_download_prepare_v1(
        torznab_instance_public_id_input,
        canonical_torrent_source_public_id_input
    );
END;
$$;
