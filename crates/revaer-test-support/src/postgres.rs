//! Helpers for launching disposable Postgres instances for integration tests without Docker.

use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

/// Handle to a disposable Postgres instance used in tests.
pub struct TestDatabase {
    connection_string: String,
    process: Option<Child>,
    data_dir: Option<PathBuf>,
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
        if let Some(process) = &mut self.process {
            let _ = process.kill();
            let _ = process.wait();
        }
        if let Some(dir) = &self.data_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }
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
    if let Ok(url) = std::env::var("REVAER_TEST_DATABASE_URL") {
        return Ok(TestDatabase {
            connection_string: url,
            process: None,
            data_dir: None,
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

    Ok(TestDatabase {
        connection_string: format!("postgres://postgres@127.0.0.1:{port}/postgres"),
        process: Some(process),
        data_dir: Some(data_dir),
    })
}

struct PostgresBinaries {
    initdb: PathBuf,
    postgres: PathBuf,
    pg_isready: PathBuf,
}

fn ensure_binaries() -> Result<PostgresBinaries> {
    let initdb = resolve_binary("initdb")?;
    let postgres = resolve_binary("postgres")?;
    let pg_isready = resolve_binary("pg_isready")?;
    Ok(PostgresBinaries {
        initdb,
        postgres,
        pg_isready,
    })
}

fn resolve_binary(name: &str) -> Result<PathBuf> {
    let mut search_paths: Vec<PathBuf> = Vec::new();
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
    let base = std::env::temp_dir();
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
