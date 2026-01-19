//! Helpers for launching disposable Postgres instances for integration tests without Docker.

use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use postgres::NoTls;
use url::Url;

/// Handle to a disposable Postgres instance used in tests.
pub struct TestDatabase {
    connection_string: String,
    process: Option<Child>,
    data_dir: Option<PathBuf>,
    cleanup: Option<DbCleanup>,
}

impl TestDatabase {
    /// Connection string that can be passed to `sqlx` or other Postgres clients.
    #[must_use]
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        if let Some(cleanup) = &self.cleanup {
            let _ = drop_database(cleanup);
        }
        if let Some(process) = &mut self.process {
            let _ = process.kill();
            let _ = process.wait();
        }
        if let Some(dir) = &self.data_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

struct DbCleanup {
    admin_url: String,
    database: String,
}

/// Start a disposable Postgres instance.
///
/// This prefers an externally supplied connection string via
/// `REVAER_TEST_DATABASE_URL`. When unset, it will attempt to use locally
/// available Postgres binaries (`initdb`, `postgres`, `pg_isready`) to spawn a
/// temporary instance. Tests can decide whether to skip when this helper
/// returns an error.
///
/// # Errors
///
/// Returns an error if no external URL is provided and Postgres binaries are
/// unavailable or fail to start.
pub fn start_postgres() -> Result<TestDatabase> {
    start_postgres_with_url(std::env::var("REVAER_TEST_DATABASE_URL").ok())
}

fn start_postgres_with_url(database_url: Option<String>) -> Result<TestDatabase> {
    if let Some(url) = database_url {
        let created = create_unique_database(&url)?;
        return Ok(TestDatabase {
            connection_string: created.connection_string,
            process: None,
            data_dir: None,
            cleanup: Some(DbCleanup {
                admin_url: created.admin_url,
                database: created.database,
            }),
        });
    }

    start_local_postgres()
}

fn start_local_postgres() -> Result<TestDatabase> {
    let binaries = ensure_binaries()?;

    let port = reserve_port()?;
    let data_dir = create_data_dir()?;

    let initdb_status = Command::new(&binaries.initdb)
        .args([
            "-D",
            data_dir
                .to_str()
                .context("data dir contains non-utf8 characters")?,
            "--username=postgres",
            "--auth=trust",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run initdb")?;
    if !initdb_status.success() {
        bail!("initdb exited with failure status");
    }

    let process = Command::new(&binaries.postgres)
        .args([
            "-D",
            data_dir
                .to_str()
                .context("data dir contains non-utf8 characters")?,
            "-p",
            &port.to_string(),
            "-h",
            "127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start postgres process")?;

    wait_for_ready(&binaries.pg_isready, port)?;

    let base_url = format!("postgres://postgres@127.0.0.1:{port}/postgres");
    let created = create_unique_database(&base_url)?;

    Ok(TestDatabase {
        connection_string: created.connection_string,
        process: Some(process),
        data_dir: Some(data_dir),
        cleanup: Some(DbCleanup {
            admin_url: created.admin_url,
            database: created.database,
        }),
    })
}

struct PostgresBinaries {
    initdb: PathBuf,
    postgres: PathBuf,
    pg_isready: PathBuf,
}

fn ensure_binaries() -> Result<PostgresBinaries> {
    let search_paths = resolve_search_paths();
    ensure_binaries_in_paths(&search_paths)
}

fn ensure_binaries_in_paths(search_paths: &[PathBuf]) -> Result<PostgresBinaries> {
    let initdb = resolve_binary_in_paths("initdb", search_paths)?;
    let postgres = resolve_binary_in_paths("postgres", search_paths)?;
    let pg_isready = resolve_binary_in_paths("pg_isready", search_paths)?;
    Ok(PostgresBinaries {
        initdb,
        postgres,
        pg_isready,
    })
}

fn resolve_search_paths() -> Vec<PathBuf> {
    let mut search_paths = Vec::new();
    // Prefer full Postgres server installations so `initdb` has the required assets.
    search_paths.extend([
        PathBuf::from("/opt/homebrew/opt/postgresql@16/bin"),
        PathBuf::from("/usr/local/opt/postgresql@16/bin"),
    ]);
    search_paths.extend(
        std::env::var_os("PATH")
            .map_or_else(Vec::new, |paths| std::env::split_paths(&paths).collect()),
    );
    search_paths.extend([
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
    ]);
    search_paths
}

fn resolve_binary_in_paths(name: &str, search_paths: &[PathBuf]) -> Result<PathBuf> {
    for dir in search_paths {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!("{name} binary is required for Postgres tests");
}

fn reserve_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("failed to reserve port")?;
    let port = listener
        .local_addr()
        .context("failed to read listener address")?
        .port();
    drop(listener);
    Ok(port)
}

fn create_data_dir() -> Result<PathBuf> {
    let base = PathBuf::from(".server_root/postgres");
    fs::create_dir_all(&base)
        .with_context(|| format!("failed to create base dir {}", base.display()))?;
    for attempt in 0..5 {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = base.join(format!("revaer-pg-{suffix}-{attempt}"));
        if !candidate.exists() {
            fs::create_dir_all(&candidate)
                .with_context(|| format!("failed to create data dir {}", candidate.display()))?;
            return Ok(candidate);
        }
    }
    bail!("failed to allocate temporary data directory for postgres");
}

fn wait_for_ready(pg_isready: &PathBuf, port: u16) -> Result<()> {
    for _ in 0..30 {
        let status = Command::new(pg_isready)
            .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-U", "postgres"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if matches!(status, Ok(ref s) if s.success()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }

    bail!("postgres process did not become ready in time")
}

struct CreatedDatabase {
    connection_string: String,
    admin_url: String,
    database: String,
}

fn create_unique_database(base_url: &str) -> Result<CreatedDatabase> {
    let parsed = Url::parse(base_url).context("invalid postgres connection url")?;
    let db_name = unique_database_name();

    let mut database_url = parsed.clone();
    database_url.set_path(&format!("/{db_name}"));

    let admin_candidates = admin_urls(&parsed);
    let mut last_error: Option<anyhow::Error> = None;
    for admin_url in admin_candidates {
        match create_database(&admin_url, &db_name) {
            Ok(()) => {
                return Ok(CreatedDatabase {
                    connection_string: database_url.to_string(),
                    admin_url,
                    database: db_name,
                });
            }
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("failed to create database")))
}

fn admin_urls(base: &Url) -> Vec<String> {
    let mut urls = Vec::new();
    let mut admin = base.clone();
    admin.set_path("/postgres");
    urls.push(admin.to_string());
    // Try the provided database as a fallback if connecting to `postgres` fails.
    if admin.path() != base.path() {
        urls.push(base.to_string());
    }
    urls
}

fn create_database(admin_url: &str, db_name: &str) -> Result<()> {
    let admin = admin_url.to_string();
    let name = db_name.to_string();
    std::thread::spawn(move || -> Result<()> {
        let config = postgres::Config::from_str(&admin)?;
        let mut client = config.connect(NoTls)?;
        client
            .simple_query(&format!("CREATE DATABASE \"{name}\""))
            .map(|_| ())
            .context("failed to issue CREATE DATABASE")
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("create database thread panicked")))?;
    Ok(())
}

fn drop_database(cleanup: &DbCleanup) -> Result<()> {
    let admin = cleanup.admin_url.clone();
    let name = cleanup.database.clone();
    std::thread::spawn(move || -> Result<()> {
        let config = postgres::Config::from_str(&admin)?;
        let mut client = config.connect(NoTls)?;
        client
            .simple_query(&format!("DROP DATABASE IF EXISTS \"{name}\""))
            .map(|_| ())
            .context("failed to drop test database")
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("drop database thread panicked")))?;
    Ok(())
}

fn unique_database_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    format!("revaer_test_{pid}_{nanos}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!("{label}-{}", unique_database_name());
        path.push(unique);
        path
    }

    #[test]
    fn admin_urls_includes_postgres_and_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let base = Url::parse("postgres://user@localhost:5432/revaer")?;
        let urls = admin_urls(&base);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].ends_with("/postgres"));
        assert!(urls[1].ends_with("/revaer"));
        Ok(())
    }

    #[test]
    fn admin_urls_skips_duplicate_when_already_postgres() -> Result<(), Box<dyn std::error::Error>>
    {
        let base = Url::parse("postgres://user@localhost:5432/postgres")?;
        let urls = admin_urls(&base);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].ends_with("/postgres"));
        Ok(())
    }

    #[test]
    fn unique_database_name_prefixes_with_pid() {
        let name = unique_database_name();
        let pid = std::process::id();
        assert!(name.starts_with(&format!("revaer_test_{pid}_")));
    }

    #[test]
    fn reserve_port_returns_bindable_port() -> Result<(), Box<dyn std::error::Error>> {
        let port = reserve_port()?;
        assert!(port > 0);
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        drop(listener);
        Ok(())
    }

    #[test]
    fn create_data_dir_builds_unique_directory() -> Result<(), Box<dyn std::error::Error>> {
        let path = create_data_dir()?;
        assert!(path.exists());
        assert!(path.ends_with("postgres") || path.to_string_lossy().contains("postgres"));
        fs::remove_dir_all(&path)?;
        Ok(())
    }

    #[test]
    fn resolve_binary_returns_error_for_missing_binary() {
        let result = resolve_binary_in_paths("definitely-missing-binary", &[]);
        assert!(result.is_err(), "expected error for missing binary");
    }

    #[test]
    fn ensure_binaries_uses_path_entries() -> Result<(), Box<dyn std::error::Error>> {
        let temp = temp_dir("pg-binaries");
        fs::create_dir_all(&temp)?;
        for name in ["initdb", "postgres", "pg_isready"] {
            fs::write(temp.join(name), "")?;
        }
        let binaries = ensure_binaries_in_paths(std::slice::from_ref(&temp))?;
        fs::remove_dir_all(&temp)?;
        assert!(binaries.initdb.ends_with("initdb"));
        assert!(binaries.postgres.ends_with("postgres"));
        assert!(binaries.pg_isready.ends_with("pg_isready"));
        Ok(())
    }

    #[test]
    fn create_unique_database_rejects_invalid_url() {
        let result = create_unique_database("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn create_and_drop_database_report_connection_errors() {
        let admin_url = "postgres://127.0.0.1:0/postgres";
        let create_result = create_database(admin_url, "missing_db");
        assert!(create_result.is_err());

        let cleanup = DbCleanup {
            admin_url: admin_url.to_string(),
            database: "missing_db".to_string(),
        };
        let drop_result = drop_database(&cleanup);
        assert!(drop_result.is_err());
    }

    #[test]
    fn start_postgres_rejects_invalid_env_url() {
        let result = start_postgres_with_url(Some("not-a-url".into()));
        assert!(result.is_err());
    }
}
