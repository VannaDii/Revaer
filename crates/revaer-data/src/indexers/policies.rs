//! Stored-procedure access for policy set and rule management.
//!
//! # Design
//! - Encapsulates policy procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const POLICY_SET_CREATE_CALL: &str = r"
    SELECT policy_set_create(
        actor_user_public_id => $1,
        display_name_input => $2,
        scope_input => $3::policy_scope,
        enabled_input => $4
    )
";

const POLICY_SET_UPDATE_CALL: &str = r"
    SELECT policy_set_update(
        actor_user_public_id => $1,
        policy_set_public_id_input => $2,
        display_name_input => $3
    )
";

const POLICY_SET_ENABLE_CALL: &str = r"
    SELECT policy_set_enable(
        actor_user_public_id => $1,
        policy_set_public_id_input => $2
    )
";

const POLICY_SET_DISABLE_CALL: &str = r"
    SELECT policy_set_disable(
        actor_user_public_id => $1,
        policy_set_public_id_input => $2
    )
";

const POLICY_SET_REORDER_CALL: &str = r"
    SELECT policy_set_reorder(
        actor_user_public_id => $1,
        ordered_policy_set_public_ids => $2
    )
";

const POLICY_RULE_CREATE_CALL: &str = r"
    SELECT policy_rule_create(
        actor_user_public_id => $1,
        policy_set_public_id_input => $2,
        rule_type_input => $3::policy_rule_type,
        match_field_input => $4::policy_match_field,
        match_operator_input => $5::policy_match_operator,
        sort_order_input => $6,
        match_value_text_input => $7,
        match_value_int_input => $8,
        match_value_uuid_input => $9,
        value_set_items_input => $10::policy_rule_value_item[],
        action_input => $11::policy_action,
        severity_input => $12::policy_severity,
        is_case_insensitive_input => $13,
        rationale_input => $14,
        expires_at_input => $15
    )
";

const POLICY_RULE_DISABLE_CALL: &str = r"
    SELECT policy_rule_disable(
        actor_user_public_id => $1,
        policy_rule_public_id_input => $2
    )
";

const POLICY_RULE_ENABLE_CALL: &str = r"
    SELECT policy_rule_enable(
        actor_user_public_id => $1,
        policy_rule_public_id_input => $2
    )
";

const POLICY_RULE_REORDER_CALL: &str = r"
    SELECT policy_rule_reorder(
        actor_user_public_id => $1,
        policy_set_public_id_input => $2,
        ordered_rule_public_ids => $3
    )
";

const POLICY_SET_RULE_LIST_CALL: &str = r"
    SELECT
        policy_set_public_id,
        policy_set_display_name,
        scope::text AS scope,
        is_enabled,
        user_public_id,
        policy_rule_public_id,
        rule_type::text AS rule_type,
        match_field::text AS match_field,
        match_operator::text AS match_operator,
        sort_order,
        match_value_text,
        match_value_int,
        match_value_uuid,
        action::text AS action,
        severity::text AS severity,
        is_case_insensitive,
        rationale,
        expires_at,
        is_rule_disabled
    FROM indexer_policy_set_rule_list(
        actor_user_public_id => $1
    )
";

/// Flattened policy-set/rule inventory row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct PolicySetRuleListRow {
    /// Policy-set public identifier.
    pub policy_set_public_id: Uuid,
    /// Policy-set display name.
    pub policy_set_display_name: String,
    /// Scope key for the policy set.
    pub scope: String,
    /// Whether the set is enabled.
    pub is_enabled: bool,
    /// Optional user public identifier for user-scoped sets.
    pub user_public_id: Option<Uuid>,
    /// Optional policy-rule public identifier.
    pub policy_rule_public_id: Option<Uuid>,
    /// Optional rule type.
    pub rule_type: Option<String>,
    /// Optional match field.
    pub match_field: Option<String>,
    /// Optional match operator.
    pub match_operator: Option<String>,
    /// Optional rule sort order.
    pub sort_order: Option<i32>,
    /// Optional text match value.
    pub match_value_text: Option<String>,
    /// Optional integer match value.
    pub match_value_int: Option<i32>,
    /// Optional UUID match value.
    pub match_value_uuid: Option<Uuid>,
    /// Optional action key.
    pub action: Option<String>,
    /// Optional severity key.
    pub severity: Option<String>,
    /// Optional case-insensitive flag.
    pub is_case_insensitive: Option<bool>,
    /// Optional rationale text.
    pub rationale: Option<String>,
    /// Optional expiry timestamp.
    pub expires_at: Option<DateTime<Utc>>,
    /// Optional disabled marker for a rule.
    pub is_rule_disabled: Option<bool>,
}

/// Create a policy set.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_set_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    display_name: &str,
    scope: &str,
    enabled: Option<bool>,
) -> Result<Uuid> {
    sqlx::query_scalar(POLICY_SET_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(display_name)
        .bind(scope)
        .bind(enabled)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy set create"))
}

/// Update a policy set.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_set_update(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_set_public_id: Uuid,
    display_name: Option<&str>,
) -> Result<Uuid> {
    sqlx::query_scalar(POLICY_SET_UPDATE_CALL)
        .bind(actor_user_public_id)
        .bind(policy_set_public_id)
        .bind(display_name)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy set update"))
}

/// Enable a policy set.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_set_enable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_set_public_id: Uuid,
) -> Result<()> {
    sqlx::query(POLICY_SET_ENABLE_CALL)
        .bind(actor_user_public_id)
        .bind(policy_set_public_id)
        .execute(pool)
        .await
        .map_err(try_op("policy set enable"))?;
    Ok(())
}

/// Disable a policy set.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_set_disable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_set_public_id: Uuid,
) -> Result<()> {
    sqlx::query(POLICY_SET_DISABLE_CALL)
        .bind(actor_user_public_id)
        .bind(policy_set_public_id)
        .execute(pool)
        .await
        .map_err(try_op("policy set disable"))?;
    Ok(())
}

/// Reorder policy sets.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_set_reorder(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    ordered_policy_set_public_ids: &[Uuid],
) -> Result<()> {
    sqlx::query(POLICY_SET_REORDER_CALL)
        .bind(actor_user_public_id)
        .bind(ordered_policy_set_public_ids)
        .execute(pool)
        .await
        .map_err(try_op("policy set reorder"))?;
    Ok(())
}

/// Value-set item for policy rules that use `in_set`.
#[derive(Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "policy_rule_value_item")]
pub struct PolicyRuleValueItem {
    /// Optional text value.
    pub value_text: Option<String>,
    /// Optional integer value.
    pub value_int: Option<i32>,
    /// Optional bigint value.
    pub value_bigint: Option<i64>,
    /// Optional UUID value.
    pub value_uuid: Option<Uuid>,
}

/// Input payload for creating a policy rule.
#[derive(Debug, Clone)]
pub struct PolicyRuleCreateInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Uuid,
    /// Policy set public id.
    pub policy_set_public_id: Uuid,
    /// Rule type key.
    pub rule_type: &'a str,
    /// Match field key.
    pub match_field: &'a str,
    /// Match operator key.
    pub match_operator: &'a str,
    /// Optional sort order.
    pub sort_order: Option<i32>,
    /// Optional text match value.
    pub match_value_text: Option<&'a str>,
    /// Optional integer match value.
    pub match_value_int: Option<i32>,
    /// Optional UUID match value.
    pub match_value_uuid: Option<Uuid>,
    /// Optional value-set items for `in_set` rules.
    pub value_set_items: Option<&'a [PolicyRuleValueItem]>,
    /// Action key for the rule.
    pub action: &'a str,
    /// Severity key for the rule.
    pub severity: &'a str,
    /// Optional case-insensitivity flag.
    pub is_case_insensitive: Option<bool>,
    /// Optional rationale text.
    pub rationale: Option<&'a str>,
    /// Optional expiration timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Create a policy rule.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_rule_create(pool: &PgPool, input: &PolicyRuleCreateInput<'_>) -> Result<Uuid> {
    sqlx::query_scalar(POLICY_RULE_CREATE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.policy_set_public_id)
        .bind(input.rule_type)
        .bind(input.match_field)
        .bind(input.match_operator)
        .bind(input.sort_order)
        .bind(input.match_value_text)
        .bind(input.match_value_int)
        .bind(input.match_value_uuid)
        .bind(input.value_set_items)
        .bind(input.action)
        .bind(input.severity)
        .bind(input.is_case_insensitive)
        .bind(input.rationale)
        .bind(input.expires_at)
        .fetch_one(pool)
        .await
        .map_err(try_op("policy rule create"))
}

/// Disable a policy rule.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_rule_disable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_rule_public_id: Uuid,
) -> Result<()> {
    sqlx::query(POLICY_RULE_DISABLE_CALL)
        .bind(actor_user_public_id)
        .bind(policy_rule_public_id)
        .execute(pool)
        .await
        .map_err(try_op("policy rule disable"))?;
    Ok(())
}

/// Enable a policy rule.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_rule_enable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_rule_public_id: Uuid,
) -> Result<()> {
    sqlx::query(POLICY_RULE_ENABLE_CALL)
        .bind(actor_user_public_id)
        .bind(policy_rule_public_id)
        .execute(pool)
        .await
        .map_err(try_op("policy rule enable"))?;
    Ok(())
}

/// Reorder policy rules.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn policy_rule_reorder(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    policy_set_public_id: Uuid,
    ordered_rule_public_ids: &[Uuid],
) -> Result<()> {
    sqlx::query(POLICY_RULE_REORDER_CALL)
        .bind(actor_user_public_id)
        .bind(policy_set_public_id)
        .bind(ordered_rule_public_ids)
        .execute(pool)
        .await
        .map_err(try_op("policy rule reorder"))?;
    Ok(())
}

/// List policy sets with flattened rule rows for operator inventory flows.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the actor or query.
pub async fn policy_set_rule_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<PolicySetRuleListRow>> {
    sqlx::query_as(POLICY_SET_RULE_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("policy set rule list"))
}

#[cfg(test)]
#[path = "policies/tests.rs"]
mod tests;
