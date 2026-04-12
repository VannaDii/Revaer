INSERT INTO indexer_instance (
    indexer_instance_public_id,
    indexer_definition_id,
    display_name,
    is_enabled,
    migration_state,
    enable_rss,
    enable_automatic_search,
    enable_interactive_search,
    priority,
    trust_tier_key,
    created_by_user_id,
    updated_by_user_id
)
VALUES (
    $1,
    $2,
    $3,
    TRUE,
    $4::indexer_instance_migration_state,
    TRUE,
    TRUE,
    TRUE,
    $5,
    $6::trust_tier_key,
    $7,
    $8
)
RETURNING indexer_instance_id
