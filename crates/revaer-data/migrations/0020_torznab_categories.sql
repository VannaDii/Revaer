-- Torznab categories and tracker mappings.

CREATE TABLE IF NOT EXISTS torznab_category (
    torznab_category_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    torznab_cat_id INTEGER NOT NULL,
    name VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT torznab_category_cat_id_uq UNIQUE (torznab_cat_id)
);

CREATE TABLE IF NOT EXISTS media_domain_to_torznab_category (
    media_domain_to_torznab_category_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    media_domain_id BIGINT NOT NULL
        REFERENCES media_domain (media_domain_id),
    torznab_category_id BIGINT NOT NULL
        REFERENCES torznab_category (torznab_category_id),
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT media_domain_to_torznab_category_uq UNIQUE (
        media_domain_id,
        torznab_category_id
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS media_domain_primary_torznab_uq
ON media_domain_to_torznab_category (media_domain_id)
WHERE is_primary = TRUE;

CREATE TABLE IF NOT EXISTS tracker_category_mapping (
    tracker_category_mapping_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    indexer_definition_id BIGINT
        REFERENCES indexer_definition (indexer_definition_id),
    tracker_category INTEGER NOT NULL
        CHECK (tracker_category >= 0),
    tracker_subcategory INTEGER NOT NULL DEFAULT 0
        CHECK (tracker_subcategory >= 0),
    torznab_category_id BIGINT NOT NULL
        REFERENCES torznab_category (torznab_category_id),
    media_domain_id BIGINT
        REFERENCES media_domain (media_domain_id),
    confidence NUMERIC(4, 3) NOT NULL DEFAULT 1.0
);

CREATE UNIQUE INDEX IF NOT EXISTS tracker_category_mapping_uq
ON tracker_category_mapping (
    coalesce(indexer_definition_id, 0::BIGINT),
    tracker_category,
    tracker_subcategory
);
