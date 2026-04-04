-- Add instance-scoped tracker category overrides.

ALTER TABLE tracker_category_mapping
    ADD COLUMN IF NOT EXISTS indexer_instance_id BIGINT
        REFERENCES indexer_instance (indexer_instance_id) ON DELETE CASCADE;

DROP INDEX IF EXISTS tracker_category_mapping_uq;

CREATE UNIQUE INDEX IF NOT EXISTS tracker_category_mapping_uq
ON tracker_category_mapping (
    coalesce(indexer_instance_id, 0::BIGINT),
    coalesce(indexer_definition_id, 0::BIGINT),
    tracker_category,
    tracker_subcategory
);

CREATE INDEX IF NOT EXISTS idx_tracker_map_instance_cat_sub
    ON tracker_category_mapping (
        indexer_instance_id,
        tracker_category,
        tracker_subcategory
    )
    WHERE indexer_instance_id IS NOT NULL;

CREATE OR REPLACE FUNCTION tracker_category_mapping_upsert_v1(
    actor_user_public_id UUID,
    indexer_definition_upstream_slug_input VARCHAR,
    indexer_instance_public_id_input UUID,
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
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    instance_definition_id BIGINT;
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

    IF indexer_instance_public_id_input IS NOT NULL THEN
        SELECT indexer_instance_id, deleted_at, indexer_definition_id
        INTO instance_id, instance_deleted_at, instance_definition_id
        FROM indexer_instance
        WHERE indexer_instance_public_id = indexer_instance_public_id_input;

        IF instance_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_instance_not_found';
        END IF;

        IF instance_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_instance_deleted';
        END IF;

        IF definition_id IS NOT NULL AND definition_id <> instance_definition_id THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_scope_conflict';
        END IF;

        definition_id := instance_definition_id;
    ELSE
        instance_id := NULL;
    END IF;

    INSERT INTO tracker_category_mapping (
        indexer_definition_id,
        indexer_instance_id,
        tracker_category,
        tracker_subcategory,
        torznab_category_id,
        media_domain_id,
        confidence
    )
    VALUES (
        definition_id,
        instance_id,
        tracker_category_input,
        tracker_subcategory_input,
        torznab_category_id_value,
        media_domain_id_value,
        1.0
    )
    ON CONFLICT (
        coalesce(indexer_instance_id, 0::BIGINT),
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
    indexer_instance_public_id_input UUID,
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
        indexer_instance_public_id_input,
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
    indexer_instance_public_id_input UUID,
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
    instance_id BIGINT;
    instance_deleted_at TIMESTAMPTZ;
    instance_definition_id BIGINT;
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

    IF indexer_instance_public_id_input IS NOT NULL THEN
        SELECT indexer_instance_id, deleted_at, indexer_definition_id
        INTO instance_id, instance_deleted_at, instance_definition_id
        FROM indexer_instance
        WHERE indexer_instance_public_id = indexer_instance_public_id_input;

        IF instance_id IS NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_instance_not_found';
        END IF;

        IF instance_deleted_at IS NOT NULL THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_instance_deleted';
        END IF;

        IF definition_id IS NOT NULL AND definition_id <> instance_definition_id THEN
            RAISE EXCEPTION USING
                ERRCODE = errcode,
                MESSAGE = base_message,
                DETAIL = 'indexer_scope_conflict';
        END IF;

        definition_id := instance_definition_id;
    ELSE
        instance_id := NULL;
    END IF;

    SELECT tracker_category_mapping_id
    INTO mapping_id
    FROM tracker_category_mapping
    WHERE tracker_category = tracker_category_input
      AND tracker_subcategory = tracker_subcategory_input
      AND (
          (instance_id IS NULL AND indexer_instance_id IS NULL)
          OR indexer_instance_id = instance_id
      )
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
    indexer_instance_public_id_input UUID,
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
        indexer_instance_public_id_input,
        tracker_category_input,
        tracker_subcategory_input
    );
END;
$$;
