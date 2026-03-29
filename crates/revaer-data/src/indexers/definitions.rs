//! Stored-procedure access for indexer definitions.
//!
//! # Design
//! - Expose read-only catalog queries via stored-procedure calls.
//! - Keep Cardigann imports transactional by routing field replacement through explicit begin,
//!   per-field insert, and finalize procedure calls.
//! - Keep SQL confined to stored-procedure calls with named binds.
//! - Return enum labels as strings to avoid extra dependencies.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::{Result, try_op};

const INDEXER_DEFINITION_LIST_CALL: &str = r"
    SELECT
        upstream_source,
        upstream_slug,
        display_name,
        protocol,
        engine,
        schema_version,
        definition_hash,
        is_deprecated,
        created_at,
        updated_at
    FROM indexer_definition_list(
        actor_user_public_id => $1
    )
";

const INDEXER_DEFINITION_IMPORT_CARDIGANN_BEGIN_CALL: &str = r"
    SELECT
        upstream_source,
        upstream_slug,
        display_name,
        protocol,
        engine,
        schema_version,
        definition_hash,
        is_deprecated,
        created_at,
        updated_at
    FROM indexer_definition_import_cardigann_begin(
        actor_user_public_id_input => $1,
        upstream_slug_input => $2,
        display_name_input => $3,
        canonical_definition_text_input => $4,
        is_deprecated_input => $5
    )
";

const INDEXER_DEFINITION_IMPORT_CARDIGANN_FIELD_CALL: &str = r"
    SELECT indexer_definition_import_cardigann_field(
        actor_user_public_id_input => $1,
        upstream_slug_input => $2,
        field_name_input => $3,
        label_input => $4,
        field_type_input => $5::field_type,
        is_required_input => $6,
        is_advanced_input => $7,
        display_order_input => $8,
        default_value_plain_input => $9,
        default_value_int_input => $10,
        default_value_decimal_input => $11,
        default_value_bool_input => $12,
        option_values_input => $13,
        option_labels_input => $14
    )
";

const INDEXER_DEFINITION_IMPORT_CARDIGANN_COMPLETE_CALL: &str = r"
    SELECT
        upstream_source,
        upstream_slug,
        display_name,
        protocol,
        engine,
        schema_version,
        definition_hash,
        is_deprecated,
        created_at,
        updated_at,
        field_count,
        option_count
    FROM indexer_definition_import_cardigann_complete(
        actor_user_public_id_input => $1,
        upstream_slug_input => $2
    )
";

/// Summary row for an indexer definition.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IndexerDefinitionRow {
    /// Upstream catalog source.
    pub upstream_source: String,
    /// Upstream slug identifier.
    pub upstream_slug: String,
    /// Display name for the definition.
    pub display_name: String,
    /// Protocol label.
    pub protocol: String,
    /// Engine label.
    pub engine: String,
    /// Schema version for the definition.
    pub schema_version: i32,
    /// Canonical definition hash (sha256 hex).
    pub definition_hash: String,
    /// Flag indicating deprecation.
    pub is_deprecated: bool,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Summary row returned after importing a Cardigann definition.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ImportedIndexerDefinitionRow {
    /// Upstream catalog source.
    pub upstream_source: String,
    /// Upstream slug identifier.
    pub upstream_slug: String,
    /// Display name for the definition.
    pub display_name: String,
    /// Protocol label.
    pub protocol: String,
    /// Engine label.
    pub engine: String,
    /// Schema version for the definition.
    pub schema_version: i32,
    /// Canonical definition hash (sha256 hex).
    pub definition_hash: String,
    /// Flag indicating deprecation.
    pub is_deprecated: bool,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Total imported field rows.
    pub field_count: i32,
    /// Total imported option rows.
    pub option_count: i32,
}

/// Field metadata used during Cardigann definition imports.
#[derive(Debug, Clone, Default)]
pub struct CardigannDefinitionFieldImport<'a> {
    /// Imported field name.
    pub field_name: &'a str,
    /// Operator-facing label.
    pub label: &'a str,
    /// Normalized Revaer field type.
    pub field_type: &'a str,
    /// Whether the setting is required.
    pub is_required: bool,
    /// Whether the field is advanced.
    pub is_advanced: bool,
    /// Stable display ordering.
    pub display_order: i32,
    /// Optional plain-text default value.
    pub default_value_plain: Option<&'a str>,
    /// Optional integer default value.
    pub default_value_int: Option<i32>,
    /// Optional decimal default value encoded as text.
    pub default_value_decimal: Option<&'a str>,
    /// Optional boolean default value.
    pub default_value_bool: Option<bool>,
    /// Optional select option values.
    pub option_values: Vec<String>,
    /// Optional select option labels.
    pub option_labels: Vec<String>,
}

/// List indexer definitions from the catalog.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_definition_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<IndexerDefinitionRow>> {
    sqlx::query_as(INDEXER_DEFINITION_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer definition list"))
}

/// Begin a transactional Cardigann definition import and clear existing field metadata.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_definition_import_cardigann_begin(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_public_id: Uuid,
    upstream_slug: &str,
    display_name: &str,
    canonical_definition_text: &str,
    is_deprecated: bool,
) -> Result<IndexerDefinitionRow> {
    sqlx::query_as(INDEXER_DEFINITION_IMPORT_CARDIGANN_BEGIN_CALL)
        .bind(actor_user_public_id)
        .bind(upstream_slug)
        .bind(display_name)
        .bind(canonical_definition_text)
        .bind(is_deprecated)
        .fetch_one(tx.as_mut())
        .await
        .map_err(try_op("indexer definition import cardigann begin"))
}

/// Insert one imported Cardigann definition field into the current transaction.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_definition_import_cardigann_field(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_public_id: Uuid,
    upstream_slug: &str,
    field: &CardigannDefinitionFieldImport<'_>,
) -> Result<()> {
    sqlx::query(INDEXER_DEFINITION_IMPORT_CARDIGANN_FIELD_CALL)
        .bind(actor_user_public_id)
        .bind(upstream_slug)
        .bind(field.field_name)
        .bind(field.label)
        .bind(field.field_type)
        .bind(field.is_required)
        .bind(field.is_advanced)
        .bind(field.display_order)
        .bind(field.default_value_plain)
        .bind(field.default_value_int)
        .bind(field.default_value_decimal)
        .bind(field.default_value_bool)
        .bind((!field.option_values.is_empty()).then_some(&field.option_values))
        .bind((!field.option_labels.is_empty()).then_some(&field.option_labels))
        .execute(tx.as_mut())
        .await
        .map_err(try_op("indexer definition import cardigann field"))?;
    Ok(())
}

/// Finalize a transactional Cardigann definition import and return the import summary.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_definition_import_cardigann_complete(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_public_id: Uuid,
    upstream_slug: &str,
) -> Result<ImportedIndexerDefinitionRow> {
    sqlx::query_as(INDEXER_DEFINITION_IMPORT_CARDIGANN_COMPLETE_CALL)
        .bind(actor_user_public_id)
        .bind(upstream_slug)
        .fetch_one(tx.as_mut())
        .await
        .map_err(try_op("indexer definition import cardigann complete"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }

    #[tokio::test]
    async fn indexer_definition_list_allows_empty_catalog() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let definitions = indexer_definition_list(pool, actor).await?;

        for definition in &definitions {
            assert!(!definition.upstream_source.is_empty());
            assert!(!definition.upstream_slug.is_empty());
            assert!(!definition.display_name.is_empty());
            assert_eq!(definition.definition_hash.len(), 64);
        }

        Ok(())
    }

    #[tokio::test]
    async fn indexer_definition_list_requires_actor() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let err = indexer_definition_list(pool, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("actor_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn cardigann_definition_import_round_trips_summary() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let mut tx = pool.begin().await?;

        indexer_definition_import_cardigann_begin(
            &mut tx,
            actor,
            "example-cardigann",
            "Example Cardigann",
            "{\"id\":\"example-cardigann\"}",
            false,
        )
        .await?;
        indexer_definition_import_cardigann_field(
            &mut tx,
            actor,
            "example-cardigann",
            &CardigannDefinitionFieldImport {
                field_name: "apikey",
                label: "API key",
                field_type: "password",
                is_required: true,
                is_advanced: false,
                display_order: 10,
                ..CardigannDefinitionFieldImport::default()
            },
        )
        .await?;
        indexer_definition_import_cardigann_field(
            &mut tx,
            actor,
            "example-cardigann",
            &CardigannDefinitionFieldImport {
                field_name: "sort",
                label: "Sort",
                field_type: "select_single",
                is_required: false,
                is_advanced: true,
                display_order: 20,
                option_values: vec!["seeders".to_string(), "date".to_string()],
                option_labels: vec!["Seeders".to_string(), "Date".to_string()],
                ..CardigannDefinitionFieldImport::default()
            },
        )
        .await?;
        let summary =
            indexer_definition_import_cardigann_complete(&mut tx, actor, "example-cardigann")
                .await?;
        tx.commit().await?;

        assert_eq!(summary.upstream_source, "cardigann");
        assert_eq!(summary.upstream_slug, "example-cardigann");
        assert_eq!(summary.engine, "cardigann");
        assert_eq!(summary.field_count, 2);
        assert_eq!(summary.option_count, 2);

        Ok(())
    }
}
