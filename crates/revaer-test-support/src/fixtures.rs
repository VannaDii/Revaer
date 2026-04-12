//! Test fixtures and environment helpers.

use std::{path::Path, process::Command};

#[doc = "Returns `true` if a Docker daemon is reachable for integration tests."]
#[must_use]
#[rustfmt::skip]
pub fn docker_available() -> bool { docker_available_with_host(std::env::var("DOCKER_HOST").ok().as_deref(), Path::new("/var/run/docker.sock")) }

#[doc = "Returns `true` if Docker is reachable via the provided host or fallback socket path."]
#[must_use]
#[rustfmt::skip]
pub fn docker_available_with_host(host: Option<&str>, default_socket: &Path) -> bool { if let Some(host) = host { return host.strip_prefix("unix://").is_none_or(|path| Path::new(path).exists()); } default_socket.exists() || Command::new("docker").args(["info"]).output().map(|output| output.status.success()).unwrap_or(false) }
