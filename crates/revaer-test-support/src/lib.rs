#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
#![allow(clippy::module_name_repetitions, clippy::multiple_crate_versions)]
#![allow(unexpected_cfgs)]

//! Shared test helpers used across integration suites.

/// Docker-related helpers for integration tests that rely on a container runtime.
pub mod docker {
    use std::path::Path;
    use std::process::Command;

    /// Returns `true` if a Docker daemon is reachable for integration tests.
    #[must_use]
    pub fn available() -> bool {
        if let Ok(host) = std::env::var("DOCKER_HOST") {
            if let Some(path) = host.strip_prefix("unix://") {
                return Path::new(path).exists();
            }
            return true;
        }

        Path::new("/var/run/docker.sock").exists()
            || Command::new("docker")
                .args(["info"])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
    }
}
