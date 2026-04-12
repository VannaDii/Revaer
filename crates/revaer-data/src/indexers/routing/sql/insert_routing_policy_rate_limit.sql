INSERT INTO routing_policy_rate_limit (routing_policy_id, rate_limit_policy_id)
SELECT rp.routing_policy_id, rlp.rate_limit_policy_id
FROM routing_policy rp
CROSS JOIN rate_limit_policy rlp
WHERE rp.routing_policy_public_id = $1
  AND rlp.rate_limit_policy_public_id = $2
