//! Test fixtures and environment helpers.

use std::path::Path;
use std::process::Command;

/// Returns `true` if a Docker daemon is reachable for integration tests.
#[must_use]
pub fn docker_available() -> bool {
    docker_available_with_host(std::env::var("DOCKER_HOST").ok())
}

fn docker_available_with_host(host: Option<String>) -> bool {
    if let Some(host) = host {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn docker_available_respects_unix_socket_env() {
        assert!(!docker_available_with_host(Some(
            "unix:///definitely/missing.sock".into()
        )));
    }

    #[test]
    fn docker_available_accepts_tcp_env() {
        assert!(docker_available_with_host(Some(
            "tcp://127.0.0.1:2375".into()
        )));
    }
}
