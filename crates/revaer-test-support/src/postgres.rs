//! Helpers for creating disposable databases on an externally managed Postgres instance.

use std::{
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

use anyhow::{Context, Result};
use postgres::NoTls;
use url::Url;

const TEST_DATABASE_URL_IS_REQUIRED: &str = "test database url is required";

#[doc = "Handle to a disposable Postgres database used in tests."]
#[derive(Debug)]
#[rustfmt::skip]
pub struct TestDatabase { connection_string: String, admin_url: String, database: String }

impl TestDatabase {
    #[doc = "Connection string that can be passed to `sqlx` or other Postgres clients."]
    #[must_use]
    #[rustfmt::skip]
    pub fn connection_string(&self) -> &str { &self.connection_string }
}

#[rustfmt::skip]
impl Drop for TestDatabase { fn drop(&mut self) { let _ = run_admin_operation(&self.admin_url, &format!("DROP DATABASE IF EXISTS \"{}\"", self.database), "failed to drop test database"); } }

#[doc = "Start a disposable test database on an externally managed Postgres instance."]
#[doc = ""]
#[doc = "# Errors"]
#[doc = "Returns an error when no test database URL is configured or provisioning fails."]
#[rustfmt::skip]
pub fn start_postgres() -> Result<TestDatabase> { std::env::var("REVAER_TEST_DATABASE_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok()).context(TEST_DATABASE_URL_IS_REQUIRED).and_then(|url| start_postgres_at(&url)) }

#[doc = "Start a disposable test database using an explicit Postgres base URL."]
#[doc = ""]
#[doc = "# Errors"]
#[doc = "Returns an error when the URL is invalid or the database cannot be created and probed."]
#[rustfmt::skip]
pub fn start_postgres_at(base_url: &str) -> Result<TestDatabase> { let parsed = Url::parse(base_url).context("invalid postgres connection url")?; let database = unique_database_name(); let connection_string = database_connection_string(&parsed, &database); let create_sql = format!("CREATE DATABASE \"{database}\""); let mut last_error = anyhow::Error::msg("failed to create database"); for admin_url in admin_urls(&parsed) { if let Err(err) = run_admin_operation(&admin_url, &create_sql, "failed to issue CREATE DATABASE") { last_error = err; continue; } run_admin_operation(&connection_string, "SELECT 1", "failed to probe test database")?; return Ok(TestDatabase { connection_string, admin_url, database }); } Err(last_error.context("failed to create database")) }

#[must_use]
#[rustfmt::skip]
fn unique_database_name() -> String { static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(1); format!("revaer_test_{}_{}", std::process::id(), NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed)) }

#[rustfmt::skip]
fn database_connection_string(base_url: &Url, database: &str) -> String { let mut database_url = base_url.clone(); database_url.set_path(&format!("/{database}")); database_url.to_string() }

#[rustfmt::skip]
fn admin_urls(base_url: &Url) -> Vec<String> { let mut admin_url = base_url.clone(); admin_url.set_path("/postgres"); if admin_url.path() == base_url.path() { vec![admin_url.to_string()] } else { vec![admin_url.to_string(), base_url.to_string()] } }

#[rustfmt::skip]
fn run_admin_operation(connection_string: &str, sql: &str, error_context: &'static str) -> Result<()> { let connection_string = connection_string.to_owned(); let sql = sql.to_owned(); thread::spawn(move || { let config = postgres::Config::from_str(&connection_string)?; let mut client = config.connect(NoTls)?; client.simple_query(&sql).map(|_| ()).context(error_context) }).join().map_err(|_| anyhow::Error::msg("postgres admin worker panicked"))? }
