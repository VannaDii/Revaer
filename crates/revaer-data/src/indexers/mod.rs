//! Indexer data access helpers.
//!
//! # Design
//! - Organize stored-procedure access by responsibility (executor handoff, secrets, etc).
//! - Keep SQL confined to stored-procedure calls with explicit parameter binding.
//! - Favor small, focused modules to avoid grab-bag helpers.

pub mod app_users;
pub mod backup;
pub mod canonical;
pub mod category_mapping;
pub mod cf_state;
pub mod conflicts;
pub mod connectivity;
pub mod definitions;
pub mod deployment;
pub mod executor;
pub mod import_jobs;
pub mod instances;
pub mod jobs;
pub mod normalization;
pub mod notifications;
pub mod outbound_requests;
pub mod policies;
pub mod policy_match;
pub mod rate_limits;
pub mod routing;
pub mod rss;
pub mod search_pages;
pub mod search_profiles;
pub mod search_requests;
pub mod search_results;
pub mod secrets;
pub mod tags;
pub mod torznab;

#[cfg(test)]
use chrono::{DateTime, Utc};
#[cfg(test)]
use revaer_test_support::postgres::start_postgres;
#[cfg(test)]
use sqlx::PgPool;
#[cfg(test)]
use sqlx::postgres::PgPoolOptions;

/// Shared test database handle for indexer stored-procedure tests.
#[cfg(test)]
pub(crate) struct IndexerTestDb {
    _db: revaer_test_support::postgres::TestDatabase,
    pool: PgPool,
    now: DateTime<Utc>,
}

#[cfg(test)]
impl IndexerTestDb {
    /// Access the underlying connection pool.
    pub(crate) const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Deterministic clock value captured after migrations.
    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

/// Create a seeded Postgres database for indexer stored-proc tests.
#[cfg(test)]
pub(crate) async fn setup_indexer_db(label: &str) -> anyhow::Result<IndexerTestDb> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping {label}: {err}");
            return Err(anyhow::anyhow!("postgres unavailable"));
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET TIME ZONE 'UTC'")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('revaer.secret_key_id', 'test-key', false)")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('revaer.secret_key', 'test-secret', false)")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(postgres.connection_string())
        .await?;

    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(&pool).await?;

    let now = sqlx::query_scalar("SELECT now()").fetch_one(&pool).await?;

    Ok(IndexerTestDb {
        _db: postgres,
        pool,
        now,
    })
}
