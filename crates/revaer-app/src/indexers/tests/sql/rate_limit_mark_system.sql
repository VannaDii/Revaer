UPDATE rate_limit_policy
SET is_system = TRUE
WHERE rate_limit_policy_public_id = $1
