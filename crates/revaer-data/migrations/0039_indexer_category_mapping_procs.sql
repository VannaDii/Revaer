-- Stored procedures for category mappings.

CREATE OR REPLACE FUNCTION tracker_category_mapping_upsert_v1(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    tracker_category_input INTEGER,
    tracker_subcategory_input INTEGER,
    torznab_cat_id_input INTEGER,
    media_domain_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to upsert tracker category mapping';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    definition_id BIGINT;
    torznab_category_id_value BIGINT;
    media_domain_id_value BIGINT;
    normalized_slug VARCHAR(128);
    normalized_media_domain VARCHAR(128);
    mapping_id BIGINT;
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

    IF tracker_category_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_category_missing';
    END IF;

    IF tracker_category_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_category_invalid';
    END IF;

    IF tracker_subcategory_input IS NULL THEN
        tracker_subcategory_input := 0;
    END IF;

    IF tracker_subcategory_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_subcategory_invalid';
    END IF;

    IF torznab_cat_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_missing';
    END IF;

    SELECT torznab_category_id
    INTO torznab_category_id_value
    FROM torznab_category
    WHERE torznab_cat_id = torznab_cat_id_input;

    IF torznab_category_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_not_found';
    END IF;

    IF media_domain_key_input IS NOT NULL THEN
        normalized_media_domain := lower(trim(media_domain_key_input));

        IF normalized_media_domain = '' OR normalized_media_domain <> media_domain_key_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_key_invalid';
        END IF;

        SELECT media_domain_id
        INTO media_domain_id_value
        FROM media_domain
        WHERE media_domain_key::TEXT = normalized_media_domain;

        IF media_domain_id_value IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'media_domain_not_found';
        END IF;
    ELSE
        media_domain_id_value := NULL;
    END IF;

    IF indexer_definition_upstream_slug_input IS NOT NULL THEN
        normalized_slug := lower(trim(indexer_definition_upstream_slug_input));

        IF normalized_slug = '' OR normalized_slug <> indexer_definition_upstream_slug_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_slug_invalid';
        END IF;

        SELECT indexer_definition_id
        INTO definition_id
        FROM indexer_definition
        WHERE upstream_source = 'prowlarr_indexers'
          AND upstream_slug = normalized_slug;

        IF definition_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_definition_not_found';
        END IF;
    ELSE
        definition_id := NULL;
    END IF;

    INSERT INTO tracker_category_mapping (
        indexer_definition_id,
        tracker_category,
        tracker_subcategory,
        torznab_category_id,
        media_domain_id,
        confidence
    )
    VALUES (
        definition_id,
        tracker_category_input,
        tracker_subcategory_input,
        torznab_category_id_value,
        media_domain_id_value,
        1.0
    )
    ON CONFLICT (
        coalesce(indexer_definition_id, 0::BIGINT),
        tracker_category,
        tracker_subcategory
    )
    DO UPDATE SET
        torznab_category_id = EXCLUDED.torznab_category_id,
        media_domain_id = EXCLUDED.media_domain_id,
        confidence = EXCLUDED.confidence
    RETURNING tracker_category_mapping_id INTO mapping_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'tracker_category_mapping',
        mapping_id,
        NULL,
        'update',
        actor_user_id,
        'tracker_category_mapping_upsert'
    );
END;
$$;

CREATE OR REPLACE FUNCTION tracker_category_mapping_upsert(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    tracker_category_input INTEGER,
    tracker_subcategory_input INTEGER,
    torznab_cat_id_input INTEGER,
    media_domain_key_input VARCHAR
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM tracker_category_mapping_upsert_v1(
        actor_user_public_id,
        indexer_definition_upstream_slug_input,
        tracker_category_input,
        tracker_subcategory_input,
        torznab_cat_id_input,
        media_domain_key_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION tracker_category_mapping_delete_v1(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    tracker_category_input INTEGER,
    tracker_subcategory_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete tracker category mapping';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    definition_id BIGINT;
    normalized_slug VARCHAR(128);
    mapping_id BIGINT;
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

    IF tracker_category_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_category_missing';
    END IF;

    IF tracker_category_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_category_invalid';
    END IF;

    IF tracker_subcategory_input IS NULL THEN
        tracker_subcategory_input := 0;
    END IF;

    IF tracker_subcategory_input < 0 THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'tracker_subcategory_invalid';
    END IF;

    IF indexer_definition_upstream_slug_input IS NOT NULL THEN
        normalized_slug := lower(trim(indexer_definition_upstream_slug_input));

        IF normalized_slug = '' OR normalized_slug <> indexer_definition_upstream_slug_input THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_slug_invalid';
        END IF;

        SELECT indexer_definition_id
        INTO definition_id
        FROM indexer_definition
        WHERE upstream_source = 'prowlarr_indexers'
          AND upstream_slug = normalized_slug;

        IF definition_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_definition_not_found';
        END IF;
    ELSE
        definition_id := NULL;
    END IF;

    SELECT tracker_category_mapping_id
    INTO mapping_id
    FROM tracker_category_mapping
    WHERE tracker_category = tracker_category_input
      AND tracker_subcategory = tracker_subcategory_input
      AND (
          (definition_id IS NULL AND indexer_definition_id IS NULL)
          OR indexer_definition_id = definition_id
      );

    IF mapping_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'mapping_not_found';
    END IF;

    DELETE FROM tracker_category_mapping
    WHERE tracker_category_mapping_id = mapping_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'tracker_category_mapping',
        mapping_id,
        NULL,
        'delete',
        actor_user_id,
        'tracker_category_mapping_delete'
    );
END;
$$;

CREATE OR REPLACE FUNCTION tracker_category_mapping_delete(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    tracker_category_input INTEGER,
    tracker_subcategory_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM tracker_category_mapping_delete_v1(
        actor_user_public_id,
        indexer_definition_upstream_slug_input,
        tracker_category_input,
        tracker_subcategory_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION media_domain_to_torznab_category_upsert_v1(
    actor_user_public_id UUID,
    media_domain_key_input VARCHAR,
    torznab_cat_id_input INTEGER,
    is_primary_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to upsert media domain mapping';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    media_domain_id_value BIGINT;
    torznab_category_id_value BIGINT;
    normalized_media_domain VARCHAR(128);
    mapping_id BIGINT;
    resolved_primary BOOLEAN;
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

    IF media_domain_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_missing';
    END IF;

    normalized_media_domain := lower(trim(media_domain_key_input));

    IF normalized_media_domain = '' OR normalized_media_domain <> media_domain_key_input THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_key_invalid';
    END IF;

    SELECT media_domain_id
    INTO media_domain_id_value
    FROM media_domain
    WHERE media_domain_key::TEXT = normalized_media_domain;

    IF media_domain_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_not_found';
    END IF;

    IF torznab_cat_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_missing';
    END IF;

    SELECT torznab_category_id
    INTO torznab_category_id_value
    FROM torznab_category
    WHERE torznab_cat_id = torznab_cat_id_input;

    IF torznab_category_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_not_found';
    END IF;

    resolved_primary := COALESCE(is_primary_input, FALSE);

    IF resolved_primary THEN
        UPDATE media_domain_to_torznab_category
        SET is_primary = FALSE
        WHERE media_domain_id = media_domain_id_value;
    END IF;

    INSERT INTO media_domain_to_torznab_category (
        media_domain_id,
        torznab_category_id,
        is_primary
    )
    VALUES (
        media_domain_id_value,
        torznab_category_id_value,
        resolved_primary
    )
    ON CONFLICT (media_domain_id, torznab_category_id)
    DO UPDATE SET is_primary = EXCLUDED.is_primary
    RETURNING media_domain_to_torznab_category_id INTO mapping_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'media_domain_to_torznab_category',
        mapping_id,
        NULL,
        'update',
        actor_user_id,
        'media_domain_mapping_upsert'
    );
END;
$$;

CREATE OR REPLACE FUNCTION media_domain_to_torznab_category_upsert(
    actor_user_public_id UUID,
    media_domain_key_input VARCHAR,
    torznab_cat_id_input INTEGER,
    is_primary_input BOOLEAN
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM media_domain_to_torznab_category_upsert_v1(
        actor_user_public_id,
        media_domain_key_input,
        torznab_cat_id_input,
        is_primary_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION media_domain_to_torznab_category_delete_v1(
    actor_user_public_id UUID,
    media_domain_key_input VARCHAR,
    torznab_cat_id_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to delete media domain mapping';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    actor_role deployment_role;
    media_domain_id_value BIGINT;
    torznab_category_id_value BIGINT;
    normalized_media_domain VARCHAR(128);
    mapping_id BIGINT;
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

    IF media_domain_key_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_missing';
    END IF;

    normalized_media_domain := lower(trim(media_domain_key_input));

    IF normalized_media_domain = '' OR normalized_media_domain <> media_domain_key_input THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_key_invalid';
    END IF;

    SELECT media_domain_id
    INTO media_domain_id_value
    FROM media_domain
    WHERE media_domain_key::TEXT = normalized_media_domain;

    IF media_domain_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'media_domain_not_found';
    END IF;

    IF torznab_cat_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_missing';
    END IF;

    SELECT torznab_category_id
    INTO torznab_category_id_value
    FROM torznab_category
    WHERE torznab_cat_id = torznab_cat_id_input;

    IF torznab_category_id_value IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_category_not_found';
    END IF;

    SELECT media_domain_to_torznab_category_id
    INTO mapping_id
    FROM media_domain_to_torznab_category
    WHERE media_domain_id = media_domain_id_value
      AND torznab_category_id = torznab_category_id_value;

    IF mapping_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'mapping_not_found';
    END IF;

    DELETE FROM media_domain_to_torznab_category
    WHERE media_domain_to_torznab_category_id = mapping_id;

    INSERT INTO config_audit_log (
        entity_type,
        entity_pk_bigint,
        entity_public_id,
        action,
        changed_by_user_id,
        change_summary
    )
    VALUES (
        'media_domain_to_torznab_category',
        mapping_id,
        NULL,
        'delete',
        actor_user_id,
        'media_domain_mapping_delete'
    );
END;
$$;

CREATE OR REPLACE FUNCTION media_domain_to_torznab_category_delete(
    actor_user_public_id UUID,
    media_domain_key_input VARCHAR,
    torznab_cat_id_input INTEGER
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM media_domain_to_torznab_category_delete_v1(
        actor_user_public_id,
        media_domain_key_input,
        torznab_cat_id_input
    );
END;
$$;
