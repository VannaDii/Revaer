INSERT INTO torznab_instance (
    search_profile_id,
    torznab_instance_public_id,
    display_name,
    api_key_hash,
    is_enabled
)
VALUES ($1, $2, $3, $4, TRUE)
