-- Torznab instance authentication and category listing.

CREATE OR REPLACE FUNCTION torznab_instance_authenticate_v1(
    torznab_instance_public_id_input UUID,
    api_key_plaintext_input VARCHAR
)
RETURNS TABLE(
    torznab_instance_id BIGINT,
    search_profile_id BIGINT,
    display_name VARCHAR(256)
)
LANGUAGE plpgsql
AS $$
DECLARE
    base_message CONSTANT text := 'Failed to authenticate torznab instance';
    errcode CONSTANT text := 'P0001';
    instance_id_value BIGINT;
    profile_id_value BIGINT;
    instance_enabled BOOLEAN;
    instance_deleted_at TIMESTAMPTZ;
    api_key_hash_value TEXT;
    display_name_value VARCHAR(256);
BEGIN
    IF torznab_instance_public_id_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'torznab_instance_missing';
    END IF;

    IF api_key_plaintext_input IS NULL THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'api_key_missing';
    END IF;

    SELECT instance.torznab_instance_id,
           instance.search_profile_id,
           instance.api_key_hash,
           instance.is_enabled,
           instance.deleted_at,
           instance.display_name
    INTO instance_id_value,
         profile_id_value,
         api_key_hash_value,
         instance_enabled,
         instance_deleted_at,
         display_name_value
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

    IF api_key_hash_value IS NULL OR api_key_hash_value = '' THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'api_key_hash_missing';
    END IF;

    IF crypt(api_key_plaintext_input, api_key_hash_value) <> api_key_hash_value THEN
        RAISE EXCEPTION USING
            ERRCODE = errcode,
            MESSAGE = base_message,
            DETAIL = 'api_key_invalid';
    END IF;

    torznab_instance_id := instance_id_value;
    search_profile_id := profile_id_value;
    display_name := display_name_value;
    RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION torznab_instance_authenticate(
    torznab_instance_public_id_input UUID,
    api_key_plaintext_input VARCHAR
)
RETURNS TABLE(
    torznab_instance_id BIGINT,
    search_profile_id BIGINT,
    display_name VARCHAR(256)
)
LANGUAGE plpgsql
AS $$
BEGIN
    RETURN QUERY
    SELECT * FROM torznab_instance_authenticate_v1(
        torznab_instance_public_id_input,
        api_key_plaintext_input
    );
END;
$$;

CREATE OR REPLACE FUNCTION torznab_category_list_v1()
RETURNS TABLE(
    torznab_cat_id INTEGER,
    name VARCHAR(128)
)
LANGUAGE sql
AS $$
    SELECT torznab_cat_id, name
    FROM torznab_category
    ORDER BY torznab_cat_id ASC;
$$;

CREATE OR REPLACE FUNCTION torznab_category_list()
RETURNS TABLE(
    torznab_cat_id INTEGER,
    name VARCHAR(128)
)
LANGUAGE sql
AS $$
    SELECT torznab_cat_id, name
    FROM torznab_category_list_v1();
$$;
