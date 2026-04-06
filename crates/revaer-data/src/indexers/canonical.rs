//! Stored-procedure access for canonical maintenance operations.
//!
//! # Design
//! - Encapsulates canonical maintenance procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const CANONICAL_MERGE_BY_INFOHASH_CALL: &str = r"
    SELECT canonical_merge_by_infohash(
        infohash_v2_input => $1,
        infohash_v1_input => $2
    )
";

const CANONICAL_RECOMPUTE_BEST_SOURCE_CALL: &str = r"
    SELECT canonical_recompute_best_source(
        canonical_torrent_public_id_input => $1,
        scoring_context_input => $2::scoring_context
    )
";

const CANONICAL_PRUNE_LOW_CONFIDENCE_CALL: &str = r"
    SELECT canonical_prune_low_confidence()
";

const CANONICAL_DISAMBIGUATION_RULE_CREATE_CALL: &str = r"
    SELECT canonical_disambiguation_rule_create(
        actor_user_public_id => $1,
        rule_type_input => $2::disambiguation_rule_type,
        identity_left_type_input => $3::disambiguation_identity_type,
        identity_left_value_text_input => $4,
        identity_left_value_uuid_input => $5,
        identity_right_type_input => $6::disambiguation_identity_type,
        identity_right_value_text_input => $7,
        identity_right_value_uuid_input => $8,
        reason_input => $9
    )
";

/// Input payload for creating a canonical disambiguation rule.
#[derive(Debug, Clone, Copy)]
pub struct CanonicalDisambiguationRuleCreateInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Uuid,
    /// Rule type key.
    pub rule_type: &'a str,
    /// Left identity type key.
    pub identity_left_type: &'a str,
    /// Optional left identity text value.
    pub identity_left_value_text: Option<&'a str>,
    /// Optional left identity UUID value.
    pub identity_left_value_uuid: Option<Uuid>,
    /// Right identity type key.
    pub identity_right_type: &'a str,
    /// Optional right identity text value.
    pub identity_right_value_text: Option<&'a str>,
    /// Optional right identity UUID value.
    pub identity_right_value_uuid: Option<Uuid>,
    /// Optional user-facing reason.
    pub reason: Option<&'a str>,
}

/// Merge canonical rows by matching infohash values.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn canonical_merge_by_infohash(
    pool: &PgPool,
    infohash_v2: Option<&str>,
    infohash_v1: Option<&str>,
) -> Result<()> {
    sqlx::query(CANONICAL_MERGE_BY_INFOHASH_CALL)
        .bind(infohash_v2)
        .bind(infohash_v1)
        .execute(pool)
        .await
        .map_err(try_op("canonical merge by infohash"))?;
    Ok(())
}

/// Recompute the best source for a canonical torrent.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn canonical_recompute_best_source(
    pool: &PgPool,
    canonical_torrent_public_id: Uuid,
    scoring_context: Option<&str>,
) -> Result<Uuid> {
    sqlx::query_scalar(CANONICAL_RECOMPUTE_BEST_SOURCE_CALL)
        .bind(canonical_torrent_public_id)
        .bind(scoring_context)
        .fetch_one(pool)
        .await
        .map_err(try_op("canonical recompute best source"))
}

/// Prune low-confidence canonical entries.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn canonical_prune_low_confidence(pool: &PgPool) -> Result<()> {
    sqlx::query(CANONICAL_PRUNE_LOW_CONFIDENCE_CALL)
        .execute(pool)
        .await
        .map_err(try_op("canonical prune low confidence"))?;
    Ok(())
}

/// Create a canonical disambiguation rule.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn canonical_disambiguation_rule_create(
    pool: &PgPool,
    input: &CanonicalDisambiguationRuleCreateInput<'_>,
) -> Result<()> {
    sqlx::query(CANONICAL_DISAMBIGUATION_RULE_CREATE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.rule_type)
        .bind(input.identity_left_type)
        .bind(input.identity_left_value_text)
        .bind(input.identity_left_value_uuid)
        .bind(input.identity_right_type)
        .bind(input.identity_right_value_text)
        .bind(input.identity_right_value_uuid)
        .bind(input.reason)
        .execute(pool)
        .await
        .map_err(try_op("canonical disambiguation rule create"))?;
    Ok(())
}

#[cfg(test)]
#[path = "canonical/tests.rs"]
mod tests;
