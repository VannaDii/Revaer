//! Strongly typed inputs and policies exposed by the libtorrent adapter.

/// Runtime parameters applied to the libtorrent session.
#[derive(Debug, Clone)]
pub struct EngineRuntimeConfig {
    /// Root directory used for new torrent data.
    pub download_root: String,
    /// Directory where fast-resume payloads are stored.
    pub resume_dir: String,
    /// Whether the distributed hash table is enabled for peer discovery.
    pub enable_dht: bool,
    /// Whether torrents default to sequential download order.
    pub sequential_default: bool,
    /// Optional listen port override for the session.
    pub listen_port: Option<i32>,
    /// Optional limit for the number of active torrents.
    pub max_active: Option<i32>,
    /// Optional global download rate limit in bytes per second.
    pub download_rate_limit: Option<i64>,
    /// Optional global upload rate limit in bytes per second.
    pub upload_rate_limit: Option<i64>,
    /// Peer encryption requirements enforced by the engine.
    pub encryption: EncryptionPolicy,
    /// Tracker configuration applied to the session.
    pub tracker: TrackerRuntimeConfig,
}

/// Supported encryption policies exposed to the orchestration layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionPolicy {
    /// Enforce encrypted peers exclusively.
    Require,
    /// Prefer encrypted peers but permit plaintext fallback.
    Prefer,
    /// Disable encrypted connections entirely.
    Disable,
}

impl EncryptionPolicy {
    #[must_use]
    /// Convert the policy to the numeric representation expected by libtorrent.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Require => 0,
            Self::Prefer => 1,
            Self::Disable => 2,
        }
    }
}

/// Proxy types supported by libtorrent for tracker announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackerProxyType {
    /// HTTP proxy.
    #[default]
    Http,
    /// HTTPS proxy.
    Https,
    /// SOCKS5 proxy.
    Socks5,
}

/// Proxy configuration for tracker communication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackerProxyRuntime {
    /// Proxy host.
    pub host: String,
    /// Proxy port.
    pub port: u16,
    /// Optional username secret reference.
    pub username_secret: Option<String>,
    /// Optional password secret reference.
    pub password_secret: Option<String>,
    /// Proxy kind.
    pub kind: TrackerProxyType,
    /// Whether peer connections should also use the proxy.
    pub proxy_peers: bool,
}

/// Tracker configuration applied to the session.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackerRuntimeConfig {
    /// Default tracker list applied to all torrents.
    pub default: Vec<String>,
    /// Extra trackers appended to defaults.
    pub extra: Vec<String>,
    /// Whether request-provided trackers replace defaults.
    pub replace: bool,
    /// Optional custom user-agent.
    pub user_agent: Option<String>,
    /// Optional announce IP override.
    pub announce_ip: Option<String>,
    /// Optional listen interface override.
    pub listen_interface: Option<String>,
    /// Optional request timeout in milliseconds.
    pub request_timeout_ms: Option<i64>,
    /// Whether to announce to all trackers.
    pub announce_to_all: bool,
    /// Optional proxy configuration.
    pub proxy: Option<TrackerProxyRuntime>,
}

#[cfg(test)]
mod tests {
    use super::EncryptionPolicy;

    #[test]
    fn encryption_policy_maps_to_expected_values() {
        assert_eq!(EncryptionPolicy::Require.as_u8(), 0);
        assert_eq!(EncryptionPolicy::Prefer.as_u8(), 1);
        assert_eq!(EncryptionPolicy::Disable.as_u8(), 2);
    }
}
