//! Stored-procedure access for policy set and rule management.
//!
//! # Design
//! - Encapsulates policy procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }

    async fn create_policy_set_and_rule(
        pool: &PgPool,
        actor: Uuid,
    ) -> anyhow::Result<(Uuid, Uuid)> {
        let policy_set_public_id =
            policy_set_create(pool, actor, "Test Policy Set", "global", Some(true)).await?;
        let input = PolicyRuleCreateInput {
            actor_user_public_id: actor,
            policy_set_public_id,
            rule_type: "block_title_regex",
            match_field: "title",
            match_operator: "regex",
            sort_order: None,
            match_value_text: Some("sample"),
            match_value_int: None,
            match_value_uuid: None,
            value_set_items: None,
            action: "drop_canonical",
            severity: "hard",
            is_case_insensitive: Some(true),
            rationale: None,
            expires_at: None,
        };
        let policy_rule_public_id = policy_rule_create(pool, &input).await?;
        Ok((policy_set_public_id, policy_rule_public_id))
    }
    #[tokio::test]
    async fn policy_set_create_rejects_duplicate_global() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let first = policy_set_create(pool, actor, "Global", "global", Some(true)).await?;
        assert_ne!(first, Uuid::nil());

        let err = policy_set_create(pool, actor, "Global", "global", Some(true))
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("global_policy_set_exists"));
        Ok(())
    }
    #[tokio::test]
    async fn policy_rule_create_requires_policy_set() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let input = PolicyRuleCreateInput {
            actor_user_public_id: actor,
            policy_set_public_id: Uuid::new_v4(),
            rule_type: "block_title_regex",
            match_field: "title",
            match_operator: "regex",
            sort_order: None,
            match_value_text: Some("sample"),
            match_value_int: None,
            match_value_uuid: None,
            value_set_items: None,
            action: "drop_canonical",
            severity: "hard",
            is_case_insensitive: Some(true),
            rationale: None,
            expires_at: None,
        };
        let err = policy_rule_create(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("policy_set_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn policy_set_reorder_requires_ids() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = policy_set_reorder(pool, actor, &[]).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("policy_set_ids_empty"));
        Ok(())
    }

    #[tokio::test]
    async fn policy_rule_disable_enable_toggles_disabled_state() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (_, policy_rule_public_id) = create_policy_set_and_rule(pool, actor).await?;

        policy_rule_disable(pool, actor, policy_rule_public_id).await?;
        let disabled_after_disable: bool = sqlx::query_scalar(
            "SELECT is_disabled
             FROM policy_rule
             WHERE policy_rule_public_id = $1",
        )
        .bind(policy_rule_public_id)
        .fetch_one(pool)
        .await?;
        assert!(disabled_after_disable);

        policy_rule_enable(pool, actor, policy_rule_public_id).await?;
        let disabled_after_enable: bool = sqlx::query_scalar(
            "SELECT is_disabled
             FROM policy_rule
             WHERE policy_rule_public_id = $1",
        )
        .bind(policy_rule_public_id)
        .fetch_one(pool)
        .await?;
        assert!(!disabled_after_enable);
        Ok(())
    }

    #[tokio::test]
    async fn policy_rule_reorder_requires_ids() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (policy_set_public_id, _) = create_policy_set_and_rule(pool, actor).await?;

        let err = policy_rule_reorder(pool, actor, policy_set_public_id, &[])
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("policy_rule_ids_empty"));
        Ok(())
    }
}
