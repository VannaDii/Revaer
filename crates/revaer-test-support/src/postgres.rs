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

    initdb_data_dir(&binaries.initdb, &data_dir)?;
    let process = spawn_postgres_process(&binaries.postgres, &data_dir, port)?;

    wait_for_ready(&binaries.pg_isready, port)?;

    let base_url = local_admin_url(port);
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

fn path_to_utf8<'path>(path: &'path std::path::Path, context: &'static str) -> Result<&'path str> {
    path.to_str().context(context)
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

fn initdb_data_dir(initdb: &std::path::Path, data_dir: &std::path::Path) -> Result<()> {
    let initdb_status = Command::new(initdb)
        .args([
            "-D",
            path_to_utf8(data_dir, "data dir contains non-utf8 characters")?,
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
    Ok(())
}

fn spawn_postgres_process(
    postgres: &std::path::Path,
    data_dir: &std::path::Path,
    port: u16,
) -> Result<Child> {
    Command::new(postgres)
        .args([
            "-D",
            path_to_utf8(data_dir, "data dir contains non-utf8 characters")?,
            "-p",
            &port.to_string(),
            "-h",
            "127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start postgres process")
}

fn local_admin_url(port: u16) -> String {
    format!("postgres://postgres@127.0.0.1:{port}/postgres")
}

fn wait_for_ready(pg_isready: &std::path::Path, port: u16) -> Result<()> {
    wait_for_ready_with_retry(pg_isready, port, 30, Duration::from_millis(200))
}

fn wait_for_ready_with_retry(
    pg_isready: &std::path::Path,
    port: u16,
    attempts: u16,
    sleep_duration: Duration,
) -> Result<()> {
    for _ in 0..attempts {
        let status = Command::new(pg_isready)
            .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-U", "postgres"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if matches!(status, Ok(ref s) if s.success()) {
            return Ok(());
        }
        thread::sleep(sleep_duration);
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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::Duration;

    fn temp_dir(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = format!("{label}-{}", unique_database_name());
        path.push(unique);
        path
    }

    #[cfg(unix)]
    struct ScriptFixture {
        root: PathBuf,
        script: PathBuf,
    }

    #[cfg(unix)]
    impl ScriptFixture {
        fn new(label: &str, body: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let root = temp_dir(label);
            fs::create_dir_all(&root)?;
            let script = root.join("script.sh");
            fs::write(&script, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
            let mut permissions = fs::metadata(&script)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions)?;
            Ok(Self { root, script })
        }
    }

    #[cfg(unix)]
    impl Drop for ScriptFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
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

    #[test]
    fn ensure_binaries_reports_missing_required_binary() -> Result<(), Box<dyn std::error::Error>> {
        let temp = temp_dir("pg-binaries-partial");
        fs::create_dir_all(&temp)?;
        fs::write(temp.join("initdb"), "")?;
        fs::write(temp.join("postgres"), "")?;

        let err = match ensure_binaries_in_paths(std::slice::from_ref(&temp)) {
            Ok(_) => return Err("expected missing pg_isready error".into()),
            Err(err) => err,
        };
        fs::remove_dir_all(&temp)?;
        assert!(err.to_string().contains("pg_isready binary is required"));
        Ok(())
    }

    #[test]
    fn resolve_search_paths_includes_environment_paths() {
        let env_paths: Vec<PathBuf> = std::env::var_os("PATH")
            .map_or_else(Vec::new, |paths| std::env::split_paths(&paths).collect());
        let search_paths = resolve_search_paths();
        assert!(
            env_paths.is_empty() || search_paths.iter().any(|path| Some(path) == env_paths.first())
        );
    }

    #[test]
    fn test_database_drop_removes_data_dir() -> Result<(), Box<dyn std::error::Error>> {
        let data_dir = temp_dir("pg-data-dir");
        fs::create_dir_all(&data_dir)?;
        {
            let _db = TestDatabase {
                connection_string: "postgres://localhost/test".to_string(),
                process: None,
                data_dir: Some(data_dir.clone()),
                cleanup: None,
            };
        }
        assert!(!data_dir.exists());
        Ok(())
    }

    #[test]
    fn start_postgres_with_external_url_creates_unique_database() -> Result<()> {
        let base_url = std::env::var("REVAER_TEST_DATABASE_URL")
            .ok()
            .or_else(|| std::env::var("DATABASE_URL").ok());
        let Some(base_url) = base_url else {
            eprintln!(
                "skipping start_postgres_with_external_url_creates_unique_database: no database url configured"
            );
            return Ok(());
        };

        let db = start_postgres_with_url(Some(base_url))?;
        let config = postgres::Config::from_str(db.connection_string())?;
        let mut client = config.connect(NoTls)?;
        let row = client.query_one("SELECT current_database()", &[])?;
        let current_database: String = row.get(0);
        assert!(current_database.starts_with("revaer_test_"));
        Ok(())
    }

    #[test]
    fn local_admin_url_formats_loopback_postgres_database() {
        assert_eq!(
            local_admin_url(15432),
            "postgres://postgres@127.0.0.1:15432/postgres"
        );
    }

    #[cfg(unix)]
    #[test]
    fn initdb_data_dir_invokes_binary_with_expected_arguments(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let args_path = temp_dir("initdb-args");
        let data_dir = temp_dir("initdb-data");
        fs::create_dir_all(&data_dir)?;

        let script = ScriptFixture::new(
            "pg-initdb-success",
            &format!("printf '%s\\n' \"$@\" > '{}'\n", args_path.display()),
        )?;
        initdb_data_dir(&script.script, &data_dir)?;

        let args = fs::read_to_string(&args_path)?;
        assert!(args.contains("-D"));
        assert!(args.contains(data_dir.to_string_lossy().as_ref()));
        assert!(args.contains("--username=postgres"));
        assert!(args.contains("--auth=trust"));

        fs::remove_dir_all(&data_dir)?;
        fs::remove_file(&args_path)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn initdb_data_dir_rejects_failure_status() -> Result<(), Box<dyn std::error::Error>> {
        let script = ScriptFixture::new("pg-initdb-failure", "exit 1")?;
        let data_dir = temp_dir("initdb-failure-data");
        fs::create_dir_all(&data_dir)?;

        let err = initdb_data_dir(&script.script, &data_dir).expect_err("expected initdb failure");
        assert!(err.to_string().contains("initdb exited with failure status"));

        fs::remove_dir_all(&data_dir)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn spawn_postgres_process_invokes_binary_with_expected_arguments(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let args_path = temp_dir("postgres-args");
        let script = ScriptFixture::new(
            "pg-postgres-success",
            &format!("printf '%s\\n' \"$@\" > '{}'\nsleep 30\n", args_path.display()),
        )?;
        let data_dir = temp_dir("postgres-data");
        fs::create_dir_all(&data_dir)?;

        let mut child = spawn_postgres_process(&script.script, &data_dir, 15432)?;
        for _ in 0..500 {
            if args_path.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !args_path.exists() {
            child.kill()?;
            let _ = child.wait()?;
            return Err("postgres args file was not written in time".into());
        }

        let args = fs::read_to_string(&args_path)?;
        assert!(args.contains("-D"));
        assert!(args.contains(data_dir.to_string_lossy().as_ref()));
        assert!(args.contains("-p"));
        assert!(args.contains("15432"));
        assert!(args.contains("-h"));
        assert!(args.contains("127.0.0.1"));

        child.kill()?;
        let _ = child.wait()?;
        fs::remove_dir_all(&data_dir)?;
        fs::remove_file(&args_path)?;
        Ok(())
    }

    #[test]
    fn spawn_postgres_process_reports_missing_binary() {
        let err = spawn_postgres_process(
            std::path::Path::new("/definitely/missing/postgres"),
            std::path::Path::new("."),
            5432,
        )
        .expect_err("missing postgres binary");
        assert!(err.to_string().contains("failed to start postgres process"));
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_ready_succeeds_after_retrying_probe() -> Result<(), Box<dyn std::error::Error>> {
        let count_path = temp_dir("pg-ready-count");
        let script = ScriptFixture::new(
            "pg-ready-eventual-success",
            &format!(
                "count=0\nif [ -f '{}' ]; then count=$(cat '{}'); fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{}'\nif [ \"$count\" -ge 3 ]; then exit 0; fi\nexit 1\n",
                count_path.display(),
                count_path.display(),
                count_path.display()
            ),
        )?;

        wait_for_ready_with_retry(&script.script, 5432, 5, Duration::from_millis(1))?;

        assert_eq!(fs::read_to_string(&count_path)?, "3");
        fs::remove_file(&count_path)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_ready_reports_timeout_when_probe_never_succeeds(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let script = ScriptFixture::new("pg-ready-timeout", "exit 1")?;

        let err = wait_for_ready_with_retry(&script.script, 5432, 2, Duration::from_millis(1))
            .expect_err("expected timeout");
        assert!(err.to_string().contains("postgres process did not become ready in time"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_ready_wrapper_accepts_immediately_ready_probe(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let script = ScriptFixture::new("pg-ready-immediate-success", "exit 0")?;

        wait_for_ready(&script.script, 5432)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_database_drop_kills_child_process_and_removes_data_dir(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data_dir = temp_dir("pg-drop-data-dir");
        fs::create_dir_all(&data_dir)?;
        let mut child = Command::new("sh").args(["-c", "sleep 30"]).spawn()?;
        let pid = child.id();
        let _ = child.try_wait()?;

        {
            let _db = TestDatabase {
                connection_string: "postgres://localhost/test".to_string(),
                process: Some(child),
                data_dir: Some(data_dir.clone()),
                cleanup: Some(DbCleanup {
                    admin_url: "postgres://127.0.0.1:0/postgres".to_string(),
                    database: "missing_db".to_string(),
                }),
            };
        }

        thread::sleep(Duration::from_millis(50));
        let status = Command::new("sh")
            .args(["-c", &format!("kill -0 {pid}")])
            .status()?;
        assert!(!status.success());
        assert!(!data_dir.exists());
        Ok(())
    }

    #[test]
    fn start_postgres_uses_env_database_url() -> Result<()> {
        if std::env::var_os("REVAER_TEST_SUPPORT_CHILD_START_POSTGRES").is_some() {
            let db = start_postgres()?;
            let config = postgres::Config::from_str(db.connection_string())?;
            let mut client = config.connect(NoTls)?;
            let row = client.query_one("SELECT current_database()", &[])?;
            let current_database: String = row.get(0);
            assert!(current_database.starts_with("revaer_test_"));
            return Ok(());
        }

        let base_url = std::env::var("REVAER_TEST_DATABASE_URL")
            .ok()
            .or_else(|| std::env::var("DATABASE_URL").ok());
        let Some(base_url) = base_url else {
            eprintln!("skipping start_postgres_uses_env_database_url: no database url configured");
            return Ok(());
        };

        let output = Command::new(std::env::current_exe()?)
            .env("REVAER_TEST_DATABASE_URL", base_url)
            .env("REVAER_TEST_SUPPORT_CHILD_START_POSTGRES", "1")
            .arg("--exact")
            .arg("postgres::tests::start_postgres_uses_env_database_url")
            .arg("--nocapture")
            .output()?;

        assert!(
            output.status.success(),
            "child test failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}
