use super::*;
use crate::DataError;
use crate::indexers::instances::{
    IndexerInstanceFieldValueInput, IndexerInstanceUpdateInput, indexer_instance_create,
    indexer_instance_field_bind_secret, indexer_instance_field_set_value, indexer_instance_update,
    rss_subscription_set,
};
use crate::indexers::rate_limits::{
    indexer_instance_set_rate_limit_policy, rate_limit_policy_create,
    routing_policy_set_rate_limit_policy,
};
use crate::indexers::routing::{
    routing_policy_bind_secret, routing_policy_create, routing_policy_set_param,
};
use crate::indexers::secrets::secret_create;
use crate::indexers::tags::tag_create;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("backup tests").await
}

async fn create_definition(pool: &PgPool, slug: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO indexer_definition (
                upstream_source,
                upstream_slug,
                display_name,
                protocol,
                engine,
                schema_version,
                definition_hash,
                is_deprecated
            )
            VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)",
    )
    .bind("prowlarr_indexers")
    .bind(slug)
    .bind("Backup Definition")
    .bind("torrent")
    .bind("torznab")
    .bind(1_i32)
    .bind("b".repeat(64))
    .bind(false)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_definition_field(
    pool: &PgPool,
    slug: &str,
    field_name: &str,
    field_type: &str,
    is_required: bool,
    display_order: i32,
) -> anyhow::Result<()> {
    let definition_id: i64 = sqlx::query_scalar(
        "SELECT indexer_definition_id
             FROM indexer_definition
             WHERE upstream_slug = $1",
    )
    .bind(slug)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO indexer_definition_field (
                indexer_definition_id,
                name,
                label,
                field_type,
                is_required,
                is_advanced,
                display_order
            )
            VALUES ($1, $2, $3, $4::field_type, $5, FALSE, $6)",
    )
    .bind(definition_id)
    .bind(field_name)
    .bind(format!("Field {field_name}"))
    .bind(field_type)
    .bind(is_required)
    .bind(display_order)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_routing_fixture(
    pool: &PgPool,
    actor: Uuid,
) -> anyhow::Result<(String, String, String, Uuid, Uuid, Uuid, Uuid)> {
    let suffix = Uuid::new_v4().simple().to_string();
    let tag_key = format!("backup-{suffix}");
    let rate_limit_name = format!("Backup Limit {suffix}");
    let routing_name = format!("Backup Route {suffix}");
    let tag_public_id = tag_create(pool, actor, &tag_key, "Backup").await?;
    let route_secret = secret_create(pool, actor, "password", "proxy-pass").await?;
    let field_secret = secret_create(pool, actor, "api_key", "indexer-key").await?;
    let rate_limit_public_id =
        rate_limit_policy_create(pool, actor, &rate_limit_name, 90, 30, 3).await?;
    let routing_policy_public_id =
        routing_policy_create(pool, actor, &routing_name, "http_proxy").await?;
    routing_policy_set_rate_limit_policy(
        pool,
        actor,
        routing_policy_public_id,
        Some(rate_limit_public_id),
    )
    .await?;
    routing_policy_set_param(
        pool,
        actor,
        routing_policy_public_id,
        "proxy_host",
        Some("proxy.internal"),
        None,
        None,
    )
    .await?;
    routing_policy_bind_secret(
        pool,
        actor,
        routing_policy_public_id,
        "http_proxy_auth",
        route_secret,
    )
    .await?;

    Ok((
        tag_key,
        rate_limit_name,
        routing_name,
        tag_public_id,
        route_secret,
        field_secret,
        routing_policy_public_id,
    ))
}

async fn seed_indexer_fixture(
    pool: &PgPool,
    actor: Uuid,
    rate_limit_name: &str,
    tag_public_id: Uuid,
    field_secret: Uuid,
    routing_policy_public_id: Uuid,
) -> anyhow::Result<(String, String)> {
    let rate_limit_public_id = sqlx::query_scalar(
        "SELECT rate_limit_policy_public_id
                 FROM rate_limit_policy
                 WHERE display_name = $1",
    )
    .bind(rate_limit_name)
    .fetch_one(pool)
    .await?;
    let slug = format!("backup-{}", Uuid::new_v4().simple());
    let instance_name = format!("Backup Instance {}", Uuid::new_v4().simple());
    create_definition(pool, &slug).await?;
    insert_definition_field(pool, &slug, "base_url", "string", true, 1).await?;
    insert_definition_field(pool, &slug, "api_key", "api_key", true, 2).await?;
    let indexer_instance_public_id = indexer_instance_create(
        pool,
        actor,
        &slug,
        &instance_name,
        Some(55),
        Some("public"),
        Some(routing_policy_public_id),
    )
    .await?;
    crate::indexers::instances::indexer_instance_set_tags(
        pool,
        actor,
        indexer_instance_public_id,
        Some(&[tag_public_id]),
        None,
    )
    .await?;
    crate::indexers::instances::indexer_instance_set_media_domains(
        pool,
        actor,
        indexer_instance_public_id,
        &[String::from("tv")],
    )
    .await?;
    indexer_instance_update(
        pool,
        &IndexerInstanceUpdateInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            display_name: None,
            priority: None,
            trust_tier_key: None,
            routing_policy_public_id: None,
            is_enabled: Some(true),
            enable_rss: Some(true),
            enable_automatic_search: Some(false),
            enable_interactive_search: Some(true),
        },
    )
    .await?;
    rss_subscription_set(pool, actor, indexer_instance_public_id, true, Some(1800)).await?;
    indexer_instance_set_rate_limit_policy(
        pool,
        actor,
        indexer_instance_public_id,
        Some(rate_limit_public_id),
    )
    .await?;
    indexer_instance_field_set_value(
        pool,
        &IndexerInstanceFieldValueInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            field_name: "base_url",
            value_plain: Some("https://indexer.example"),
            value_int: None,
            value_decimal: None,
            value_bool: None,
        },
    )
    .await?;
    indexer_instance_field_bind_secret(
        pool,
        actor,
        indexer_instance_public_id,
        "api_key",
        field_secret,
    )
    .await?;

    Ok((slug, instance_name))
}

#[tokio::test]
async fn export_lists_return_flattened_rows() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let pool = db.pool();
    let (
        tag_key,
        rate_limit_name,
        routing_name,
        tag_public_id,
        route_secret,
        field_secret,
        routing_policy_public_id,
    ) = seed_routing_fixture(pool, actor).await?;
    let (slug, instance_name) = seed_indexer_fixture(
        pool,
        actor,
        &rate_limit_name,
        tag_public_id,
        field_secret,
        routing_policy_public_id,
    )
    .await?;

    let tags = indexer_backup_export_tag_list(pool, actor).await?;
    assert!(tags.iter().any(|row| row.tag_key == tag_key));

    let rate_limits = indexer_backup_export_rate_limit_policy_list(pool, actor).await?;
    assert!(
        rate_limits
            .iter()
            .any(|row| row.display_name == rate_limit_name)
    );

    let routing = indexer_backup_export_routing_policy_list(pool, actor).await?;
    assert!(routing.iter().any(|row| {
        row.display_name == routing_name
            && row.param_key.as_deref() == Some("proxy_host")
            && row.value_plain.as_deref() == Some("proxy.internal")
    }));
    assert!(routing.iter().any(|row| {
        row.display_name == routing_name
            && row.secret_public_id == Some(route_secret)
            && row.secret_type.as_deref() == Some("password")
    }));

    let instances = indexer_backup_export_indexer_instance_list(pool, actor).await?;
    assert!(instances.iter().any(|row| {
        row.display_name == instance_name
            && row.upstream_slug == slug
            && row.media_domain_key.as_deref() == Some("tv")
    }));
    assert!(instances.iter().any(|row| {
        row.display_name == instance_name && row.tag_key.as_deref() == Some(&tag_key)
    }));
    assert!(instances.iter().any(|row| {
        row.display_name == instance_name
            && row.field_name.as_deref() == Some("base_url")
            && row.value_plain.as_deref() == Some("https://indexer.example")
    }));
    assert!(instances.iter().any(|row| {
        row.display_name == instance_name
            && row.field_name.as_deref() == Some("api_key")
            && row.secret_public_id == Some(field_secret)
            && row.secret_type.as_deref() == Some("api_key")
    }));
    Ok(())
}

#[tokio::test]
async fn export_requires_authorized_actor() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };

    let err = indexer_backup_export_tag_list(db.pool(), Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("actor_not_found"));
    Ok(())
}
