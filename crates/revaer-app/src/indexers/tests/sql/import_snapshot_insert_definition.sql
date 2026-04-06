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
    $1::upstream_source,
    $2,
    $3,
    $4::protocol,
    $5::engine,
    $6,
    $7,
    $8
)
