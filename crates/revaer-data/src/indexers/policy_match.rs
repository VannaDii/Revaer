//! Stored-procedure access for policy match helpers.
//!
//! # Design
//! - Encapsulates policy matching helpers behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const POLICY_TEXT_MATCH_CALL: &str = r"
    SELECT policy_text_match(
        candidate_input => $1,
        match_operator_input => $2::policy_match_operator,
        match_value_text_input => $3,
        value_set_id_input => $4,
        is_case_insensitive_input => $5
    )
";

const POLICY_UUID_MATCH_CALL: &str = r"
    SELECT policy_uuid_match(
        candidate_input => $1,
        match_operator_input => $2::policy_match_operator,
        match_value_uuid_input => $3,
        value_set_id_input => $4
    )
";

const POLICY_INT_MATCH_CALL: &str = r"
    SELECT policy_int_match(
        candidate_input => $1,
        match_operator_input => $2::policy_match_operator,
        match_value_int_input => $3,
        value_set_id_input => $4
    )
";

const POLICY_RELEASE_GROUP_MATCH_CALL: &str = r"
    SELECT policy_release_group_match(
        canonical_torrent_id_input => $1,
        release_group_token_input => $2,
        match_operator_input => $3::policy_match_operator,
        match_value_text_input => $4,
        value_set_id_input => $5,
        is_case_insensitive_input => $6
    )
";

/// Evaluate a text-based policy match.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_text_match(
    pool: &PgPool,
    candidate: Option<&str>,
    match_operator: &str,
    match_value_text: Option<&str>,
    value_set_id: Option<i64>,
    is_case_insensitive: bool,
) -> Result<bool> {
    sqlx::query_scalar(POLICY_TEXT_MATCH_CALL)
        .bind(candidate)
        .bind(match_operator)
        .bind(match_value_text)
        .bind(value_set_id)
        .bind(is_case_insensitive)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy text match"))
}

/// Evaluate a UUID policy match.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_uuid_match(
    pool: &PgPool,
    candidate: Option<Uuid>,
    match_operator: &str,
    match_value_uuid: Option<Uuid>,
    value_set_id: Option<i64>,
) -> Result<bool> {
    sqlx::query_scalar(POLICY_UUID_MATCH_CALL)
        .bind(candidate)
        .bind(match_operator)
        .bind(match_value_uuid)
        .bind(value_set_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy uuid match"))
}

/// Evaluate an integer policy match.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_int_match(
    pool: &PgPool,
    candidate: Option<i32>,
    match_operator: &str,
    match_value_int: Option<i32>,
    value_set_id: Option<i64>,
) -> Result<bool> {
    sqlx::query_scalar(POLICY_INT_MATCH_CALL)
        .bind(candidate)
        .bind(match_operator)
        .bind(match_value_int)
        .bind(value_set_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy int match"))
}

/// Evaluate a release-group policy match.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_release_group_match(
    pool: &PgPool,
    canonical_torrent_id: i64,
    release_group_token: Option<&str>,
    match_operator: &str,
    match_value_text: Option<&str>,
    value_set_id: Option<i64>,
    is_case_insensitive: bool,
) -> Result<bool> {
    sqlx::query_scalar(POLICY_RELEASE_GROUP_MATCH_CALL)
        .bind(canonical_torrent_id)
        .bind(release_group_token)
        .bind(match_operator)
        .bind(match_value_text)
        .bind(value_set_id)
        .bind(is_case_insensitive)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy release group match"))
}

#[cfg(test)]
#[path = "../../tests/unit/indexers/policy_match_tests.rs"]
mod tests;
