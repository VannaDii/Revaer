//! Helpers for launching disposable Postgres instances via Docker for integration tests.

use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};

use crate::fixtures::docker_available;

/// Handle to a Docker-managed Postgres container.
pub struct PostgresContainer {
    id: String,
    connection_string: String,
}

impl PostgresContainer {
    /// Connection string that can be passed to `SQLx` or other Postgres clients.
    #[must_use]
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }
}

impl Drop for PostgresContainer {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.id]).status();
    }
}

/// Start a disposable Postgres container exposed on a random local port.
///
/// The container is stopped automatically when the returned handle is dropped.
///
/// # Errors
///
/// Returns an error if Docker is unavailable, the container fails to start, or
/// readiness checks time out.
pub fn start_postgres() -> Result<PostgresContainer> {
    ensure!(
        docker_available(),
        "docker is required for postgres integration tests"
    );

    let port = reserve_port()?;
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "-e",
            "POSTGRES_PASSWORD=password",
            "-e",
            "POSTGRES_USER=postgres",
            "-e",
            "POSTGRES_DB=postgres",
            "-p",
            &format!("{port}:5432"),
            "postgres:16-alpine",
        ])
        .output()
        .context("failed to start postgres container")?;
    ensure!(
        output.status.success(),
        "docker run postgres failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let id = String::from_utf8(output.stdout)
        .context("container id not utf-8")?
        .trim()
        .to_owned();
    wait_for_ready(&id)?;
    thread::sleep(Duration::from_millis(500));

    Ok(PostgresContainer {
        id,
        connection_string: format!("postgres://postgres:password@127.0.0.1:{port}/postgres"),
    })
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

fn wait_for_ready(id: &str) -> Result<()> {
    for _ in 0..30 {
        let output = Command::new("docker")
            .args(["exec", id, "pg_isready", "-U", "postgres"])
            .output();
        if matches!(output, Ok(result) if result.status.success()) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    bail!("postgres container did not become ready in time");
}
