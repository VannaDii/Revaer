INSERT INTO rate_limit_policy (
    rate_limit_policy_public_id,
    display_name,
    requests_per_minute,
    burst,
    concurrent_requests
)
VALUES ($1, $2, $3, $4, $5)
RETURNING rate_limit_policy_public_id
