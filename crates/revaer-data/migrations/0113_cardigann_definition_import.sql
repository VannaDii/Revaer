-- Add Cardigann-backed definition import procedures.

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_enum enum_value
        JOIN pg_type enum_type
          ON enum_type.oid = enum_value.enumtypid
        WHERE enum_type.typname = 'upstream_source'
          AND enum_value.enumlabel = 'cardigann'
    ) THEN
        ALTER TYPE upstream_source ADD VALUE 'cardigann';
    END IF;
END
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_begin_v1(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR,
    display_name_input VARCHAR,
    canonical_definition_text_input TEXT,
    is_deprecated_input BOOLEAN DEFAULT false
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to import Cardigann definition';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    definition_id BIGINT;
    normalized_slug VARCHAR(128);
    normalized_display_name VARCHAR(256);
BEGIN
    IF actor_user_public_id_input IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id
    INTO actor_user_id
    FROM app_user
    WHERE user_public_id = actor_user_public_id_input;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    normalized_slug := lower(btrim(coalesce(upstream_slug_input, '')));
    IF normalized_slug = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_upstream_slug_missing';
    END IF;

    normalized_display_name := btrim(coalesce(display_name_input, ''));
    IF normalized_display_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_display_name_missing';
    END IF;

    IF canonical_definition_text_input IS NULL
        OR btrim(canonical_definition_text_input) = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_canonical_text_missing';
    END IF;

    INSERT INTO indexer_definition (
        upstream_source,
        upstream_slug,
        display_name,
        protocol,
        engine,
        schema_version,
        definition_hash,
        is_deprecated
    )
    VALUES (
        'cardigann',
        normalized_slug,
        normalized_display_name,
        'torrent',
        'cardigann',
        1,
        lower(encode(digest(canonical_definition_text_input, 'sha256'), 'hex')),
        coalesce(is_deprecated_input, false)
    )
    ON CONFLICT ON CONSTRAINT indexer_definition_upstream_uq
    DO UPDATE
    SET display_name = EXCLUDED.display_name,
        protocol = EXCLUDED.protocol,
        engine = EXCLUDED.engine,
        schema_version = EXCLUDED.schema_version,
        definition_hash = EXCLUDED.definition_hash,
        is_deprecated = EXCLUDED.is_deprecated,
        updated_at = now()
    RETURNING indexer_definition.indexer_definition_id
    INTO definition_id;

    DELETE FROM indexer_definition_field_option
    WHERE indexer_definition_field_id IN (
        SELECT indexer_definition_field_id
        FROM indexer_definition_field
        WHERE indexer_definition_id = definition_id
    );

    DELETE FROM indexer_definition_field_validation
    WHERE indexer_definition_field_id IN (
        SELECT indexer_definition_field_id
        FROM indexer_definition_field
        WHERE indexer_definition_id = definition_id
    );

    DELETE FROM indexer_definition_field
    WHERE indexer_definition_id = definition_id;

    RETURN QUERY
    SELECT
        definition.upstream_source::VARCHAR,
        definition.upstream_slug,
        definition.display_name,
        definition.protocol::VARCHAR,
        definition.engine::VARCHAR,
        definition.schema_version,
        definition.definition_hash,
        definition.is_deprecated,
        definition.created_at,
        definition.updated_at
    FROM indexer_definition definition
    WHERE definition.indexer_definition_id = definition_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_begin(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR,
    display_name_input VARCHAR,
    canonical_definition_text_input TEXT,
    is_deprecated_input BOOLEAN DEFAULT false
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_definition_import_cardigann_begin_v1(
        actor_user_public_id_input,
        upstream_slug_input,
        display_name_input,
        canonical_definition_text_input,
        is_deprecated_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_field_v1(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR,
    field_name_input VARCHAR,
    label_input VARCHAR,
    field_type_input field_type,
    is_required_input BOOLEAN,
    is_advanced_input BOOLEAN,
    display_order_input INTEGER,
    default_value_plain_input VARCHAR DEFAULT NULL,
    default_value_int_input INTEGER DEFAULT NULL,
    default_value_decimal_input VARCHAR DEFAULT NULL,
    default_value_bool_input BOOLEAN DEFAULT NULL,
    option_values_input VARCHAR[] DEFAULT NULL,
    option_labels_input VARCHAR[] DEFAULT NULL
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to import Cardigann definition field';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    definition_id BIGINT;
    field_id BIGINT;
    normalized_slug VARCHAR(128);
    normalized_field_name VARCHAR(128);
    normalized_label VARCHAR(256);
BEGIN
    IF actor_user_public_id_input IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id
    INTO actor_user_id
    FROM app_user
    WHERE user_public_id = actor_user_public_id_input;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    normalized_slug := lower(btrim(coalesce(upstream_slug_input, '')));
    IF normalized_slug = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_upstream_slug_missing';
    END IF;

    SELECT indexer_definition_id
    INTO definition_id
    FROM indexer_definition definition
    WHERE definition.upstream_source = 'cardigann'
      AND definition.upstream_slug = normalized_slug;

    IF definition_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'cardigann_definition_not_found';
    END IF;

    normalized_field_name := lower(btrim(coalesce(field_name_input, '')));
    IF normalized_field_name = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_field_name_missing';
    END IF;

    normalized_label := btrim(coalesce(label_input, ''));
    IF normalized_label = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_field_label_missing';
    END IF;

    IF coalesce(array_length(option_values_input, 1), 0)
        <> coalesce(array_length(option_labels_input, 1), 0) THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_option_length_mismatch';
    END IF;

    INSERT INTO indexer_definition_field (
        indexer_definition_id,
        name,
        label,
        field_type,
        is_required,
        is_advanced,
        display_order,
        default_value_plain,
        default_value_int,
        default_value_decimal,
        default_value_bool
    )
    VALUES (
        definition_id,
        normalized_field_name,
        normalized_label,
        field_type_input,
        coalesce(is_required_input, false),
        coalesce(is_advanced_input, false),
        coalesce(display_order_input, 1000),
        default_value_plain_input,
        default_value_int_input,
        CASE
            WHEN default_value_decimal_input IS NULL
                OR btrim(default_value_decimal_input) = '' THEN NULL
            ELSE btrim(default_value_decimal_input)::NUMERIC(12, 4)
        END,
        default_value_bool_input
    )
    RETURNING indexer_definition_field_id
    INTO field_id;

    IF coalesce(array_length(option_values_input, 1), 0) > 0 THEN
        INSERT INTO indexer_definition_field_option (
            indexer_definition_field_id,
            option_value,
            option_label,
            sort_order
        )
        SELECT
            field_id,
            option_values_input[item.ordinality],
            option_labels_input[item.ordinality],
            item.ordinality
        FROM unnest(option_values_input) WITH ORDINALITY AS item(option_value, ordinality);
    END IF;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_field(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR,
    field_name_input VARCHAR,
    label_input VARCHAR,
    field_type_input field_type,
    is_required_input BOOLEAN,
    is_advanced_input BOOLEAN,
    display_order_input INTEGER,
    default_value_plain_input VARCHAR DEFAULT NULL,
    default_value_int_input INTEGER DEFAULT NULL,
    default_value_decimal_input VARCHAR DEFAULT NULL,
    default_value_bool_input BOOLEAN DEFAULT NULL,
    option_values_input VARCHAR[] DEFAULT NULL,
    option_labels_input VARCHAR[] DEFAULT NULL
)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM indexer_definition_import_cardigann_field_v1(
        actor_user_public_id_input,
        upstream_slug_input,
        field_name_input,
        label_input,
        field_type_input,
        is_required_input,
        is_advanced_input,
        display_order_input,
        default_value_plain_input,
        default_value_int_input,
        default_value_decimal_input,
        default_value_bool_input,
        option_values_input,
        option_labels_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_complete_v1(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    field_count INTEGER,
    option_count INTEGER
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to finalize Cardigann definition import';
    errcode CONSTANT text := 'P0001';
    actor_user_id BIGINT;
    definition_id BIGINT;
    normalized_slug VARCHAR(128);
BEGIN
    IF actor_user_public_id_input IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_missing';
    END IF;

    SELECT user_id
    INTO actor_user_id
    FROM app_user
    WHERE user_public_id = actor_user_public_id_input;

    IF actor_user_id IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = errcode, MESSAGE = base_message, DETAIL = 'actor_not_found';
    END IF;

    normalized_slug := lower(btrim(coalesce(upstream_slug_input, '')));
    IF normalized_slug = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'definition_upstream_slug_missing';
    END IF;

    SELECT indexer_definition_id
    INTO definition_id
    FROM indexer_definition definition
    WHERE definition.upstream_source = 'cardigann'
      AND definition.upstream_slug = normalized_slug;

    IF definition_id IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'cardigann_definition_not_found';
    END IF;

    RETURN QUERY
    SELECT
        definition.upstream_source::VARCHAR,
        definition.upstream_slug,
        definition.display_name,
        definition.protocol::VARCHAR,
        definition.engine::VARCHAR,
        definition.schema_version,
        definition.definition_hash,
        definition.is_deprecated,
        definition.created_at,
        definition.updated_at,
        (
            SELECT count(*)::INTEGER
            FROM indexer_definition_field
            WHERE indexer_definition_id = definition.indexer_definition_id
        ),
        (
            SELECT count(*)::INTEGER
            FROM indexer_definition_field_option option_row
            JOIN indexer_definition_field field_row
              ON field_row.indexer_definition_field_id = option_row.indexer_definition_field_id
            WHERE field_row.indexer_definition_id = definition.indexer_definition_id
        )
    FROM indexer_definition definition
    WHERE definition.indexer_definition_id = definition_id;
END;
$$;

CREATE OR REPLACE FUNCTION indexer_definition_import_cardigann_complete(
    actor_user_public_id_input UUID,
    upstream_slug_input VARCHAR
)
RETURNS TABLE (
    upstream_source VARCHAR,
    upstream_slug VARCHAR,
    display_name VARCHAR,
    protocol VARCHAR,
    engine VARCHAR,
    schema_version INTEGER,
    definition_hash CHAR(64),
    is_deprecated BOOLEAN,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    field_count INTEGER,
    option_count INTEGER
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT *
    FROM indexer_definition_import_cardigann_complete_v1(
        actor_user_public_id_input,
        upstream_slug_input
    );
END;
$$;
