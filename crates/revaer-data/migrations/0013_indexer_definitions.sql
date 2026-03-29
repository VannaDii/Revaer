-- Indexer ERD definitions and field metadata.

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'upstream_source') THEN
        CREATE TYPE upstream_source AS ENUM ('prowlarr_indexers');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'protocol') THEN
        CREATE TYPE protocol AS ENUM ('torrent', 'usenet');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'engine') THEN
        CREATE TYPE engine AS ENUM ('torznab', 'cardigann');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'field_type') THEN
        CREATE TYPE field_type AS ENUM (
            'string',
            'password',
            'api_key',
            'cookie',
            'token',
            'header_value',
            'number_int',
            'number_decimal',
            'bool',
            'select_single'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'validation_type') THEN
        CREATE TYPE validation_type AS ENUM (
            'min_length',
            'max_length',
            'min_value',
            'max_value',
            'regex',
            'allowed_value',
            'required_if_field_equals'
        );
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'depends_on_operator') THEN
        CREATE TYPE depends_on_operator AS ENUM ('eq', 'neq', 'in_set');
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'value_set_type') THEN
        CREATE TYPE value_set_type AS ENUM ('text', 'int', 'bigint', 'uuid');
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS indexer_definition (
    indexer_definition_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    upstream_source upstream_source NOT NULL,
    upstream_slug VARCHAR(128) NOT NULL,
    display_name VARCHAR(256) NOT NULL,
    protocol protocol NOT NULL,
    engine engine NOT NULL,
    schema_version INTEGER NOT NULL,
    definition_hash CHAR(64) NOT NULL,
    is_deprecated BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT indexer_definition_upstream_uq UNIQUE (upstream_source, upstream_slug),
    CONSTRAINT indexer_definition_slug_lc CHECK (upstream_slug = lower(upstream_slug)),
    CONSTRAINT indexer_definition_hash_lc CHECK (definition_hash = lower(definition_hash)),
    CONSTRAINT indexer_definition_hash_hex CHECK (definition_hash ~ '^[0-9a-f]{64}$')
);

CREATE TABLE IF NOT EXISTS indexer_definition_field (
    indexer_definition_field_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_definition_id BIGINT NOT NULL
        REFERENCES indexer_definition (indexer_definition_id),
    name VARCHAR(128) NOT NULL,
    label VARCHAR(256) NOT NULL,
    field_type field_type NOT NULL,
    is_required BOOLEAN NOT NULL,
    is_advanced BOOLEAN NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 1000,
    default_value_plain VARCHAR(512),
    default_value_int INTEGER,
    default_value_decimal NUMERIC(12, 4),
    default_value_bool BOOLEAN,
    CONSTRAINT indexer_definition_field_name_uq UNIQUE (indexer_definition_id, name),
    CONSTRAINT indexer_definition_field_name_lc CHECK (name = lower(name)),
    CONSTRAINT indexer_definition_field_default_single_chk CHECK (
        (
            (default_value_plain IS NOT NULL)::INT
            + (default_value_int IS NOT NULL)::INT
            + (default_value_decimal IS NOT NULL)::INT
            + (default_value_bool IS NOT NULL)::INT
        ) <= 1
    ),
    CONSTRAINT indexer_definition_field_secret_default_chk CHECK (
        field_type NOT IN ('password', 'api_key', 'cookie', 'token', 'header_value')
        OR (
            default_value_plain IS NULL
            AND default_value_int IS NULL
            AND default_value_decimal IS NULL
            AND default_value_bool IS NULL
        )
    )
);

CREATE TABLE IF NOT EXISTS indexer_definition_field_validation (
    indexer_definition_field_validation_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_definition_field_id BIGINT NOT NULL
        REFERENCES indexer_definition_field (indexer_definition_field_id),
    validation_type validation_type NOT NULL,
    int_value INTEGER,
    numeric_value NUMERIC(12, 4),
    text_value VARCHAR(512),
    text_value_norm VARCHAR(512) GENERATED ALWAYS AS (
        CASE
            WHEN text_value IS NULL THEN NULL
            WHEN validation_type = 'regex' THEN trim(text_value)
            ELSE lower(trim(text_value))
        END
    ) STORED,
    value_set_id BIGINT,
    depends_on_field_name VARCHAR(128),
    depends_on_operator depends_on_operator,
    depends_on_value_plain VARCHAR(512),
    depends_on_value_plain_norm VARCHAR(512) GENERATED ALWAYS AS (
        CASE
            WHEN depends_on_value_plain IS NULL THEN NULL
            ELSE lower(trim(depends_on_value_plain))
        END
    ) STORED,
    depends_on_value_int INTEGER,
    depends_on_value_bool BOOLEAN,
    depends_on_value_set_id BIGINT,
    CONSTRAINT indexer_definition_field_validation_depends_name_lc CHECK (
        depends_on_field_name IS NULL
        OR depends_on_field_name = lower(depends_on_field_name)
    ),
    CONSTRAINT indexer_definition_field_validation_min_len_chk CHECK (
        validation_type NOT IN ('min_length', 'max_length')
        OR (int_value IS NOT NULL AND int_value >= 0)
    ),
    CONSTRAINT indexer_definition_field_validation_min_val_chk CHECK (
        validation_type NOT IN ('min_value', 'max_value')
        OR numeric_value IS NOT NULL
    ),
    CONSTRAINT indexer_definition_field_validation_regex_chk CHECK (
        validation_type <> 'regex'
        OR text_value IS NOT NULL
    ),
    CONSTRAINT indexer_definition_field_validation_allowed_value_chk CHECK (
        validation_type <> 'allowed_value'
        OR (
            (text_value IS NOT NULL AND value_set_id IS NULL)
            OR (text_value IS NULL AND value_set_id IS NOT NULL)
        )
    ),
    CONSTRAINT indexer_definition_field_validation_required_if_chk CHECK (
        validation_type <> 'required_if_field_equals'
        OR (
            depends_on_field_name IS NOT NULL
            AND depends_on_operator IS NOT NULL
            AND (
                (
                    (depends_on_value_plain IS NOT NULL)::INT
                    + (depends_on_value_int IS NOT NULL)::INT
                    + (depends_on_value_bool IS NOT NULL)::INT
                    + (depends_on_value_set_id IS NOT NULL)::INT
                ) = 1
            )
        )
    )
);

CREATE TABLE IF NOT EXISTS indexer_definition_field_value_set (
    value_set_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_definition_field_validation_id BIGINT NOT NULL
        REFERENCES indexer_definition_field_validation (indexer_definition_field_validation_id),
    value_set_type value_set_type NOT NULL,
    name VARCHAR(256),
    CONSTRAINT indexer_definition_field_value_set_uq UNIQUE (
        indexer_definition_field_validation_id
    ),
    CONSTRAINT indexer_definition_field_value_set_type_chk CHECK (
        value_set_type IN ('text', 'int', 'bigint')
    )
);

CREATE TABLE IF NOT EXISTS indexer_definition_field_value_set_item (
    value_set_item_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    value_set_id BIGINT NOT NULL
        REFERENCES indexer_definition_field_value_set (value_set_id),
    value_text VARCHAR(256),
    value_int INTEGER,
    value_bigint BIGINT,
    CONSTRAINT indexer_definition_field_value_set_item_single_chk CHECK (
        (
            (value_text IS NOT NULL)::INT
            + (value_int IS NOT NULL)::INT
            + (value_bigint IS NOT NULL)::INT
        ) = 1
    ),
    CONSTRAINT indexer_definition_field_value_set_item_text_lc CHECK (
        value_text IS NULL OR value_text = lower(value_text)
    )
);

CREATE TABLE IF NOT EXISTS indexer_definition_field_option (
    indexer_definition_field_option_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_definition_field_id BIGINT NOT NULL
        REFERENCES indexer_definition_field (indexer_definition_field_id),
    option_value VARCHAR(128) NOT NULL,
    option_label VARCHAR(256) NOT NULL,
    sort_order INTEGER NOT NULL,
    CONSTRAINT indexer_definition_field_option_uq UNIQUE (
        indexer_definition_field_id,
        option_value
    )
);

ALTER TABLE indexer_definition_field_validation
    ADD CONSTRAINT indexer_definition_field_validation_value_set_fk
        FOREIGN KEY (value_set_id)
        REFERENCES indexer_definition_field_value_set (value_set_id);

ALTER TABLE indexer_definition_field_validation
    ADD CONSTRAINT indexer_definition_field_validation_depends_value_set_fk
        FOREIGN KEY (depends_on_value_set_id)
        REFERENCES indexer_definition_field_value_set (value_set_id);

CREATE UNIQUE INDEX IF NOT EXISTS indexer_definition_field_validation_uq
ON indexer_definition_field_validation (
    indexer_definition_field_id,
    validation_type,
    coalesce(depends_on_field_name, ''),
    coalesce(depends_on_operator, 'eq'::depends_on_operator),
    (depends_on_operator IS NULL),
    coalesce(text_value_norm, ''),
    coalesce(int_value, -1),
    coalesce(numeric_value, -1),
    coalesce(value_set_id, 0),
    coalesce(depends_on_value_set_id, 0),
    coalesce(depends_on_value_plain_norm, ''),
    coalesce(depends_on_value_int, -1),
    coalesce(depends_on_value_bool, false)
);
