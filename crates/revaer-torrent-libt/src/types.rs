//! Strongly typed inputs and policies exposed by the libtorrent adapter.

/// Wrapper for boolean flags to avoid pedantic lint churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Toggle(pub bool);

impl Toggle {
    #[must_use]
    /// Whether the toggle is enabled.
    pub const fn is_enabled(self) -> bool {
        self.0
    }
}

impl From<bool> for Toggle {
    fn from(value: bool) -> Self {
        Self(value)
    }
}

impl From<Toggle> for bool {
    fn from(toggle: Toggle) -> Self {
        toggle.0
    }
}

/// Runtime parameters applied to the libtorrent session.
#[derive(Debug, Clone)]
pub struct EngineRuntimeConfig {
    /// Root directory used for new torrent data.
    pub download_root: String,
    /// Directory where fast-resume payloads are stored.
    pub resume_dir: String,
    /// Explicit listen interfaces (host/device/IP + port).
    pub listen_interfaces: Vec<String>,
    /// IPv6 preference for listening and outbound behaviour.
    pub ipv6_mode: Ipv6Mode,
    /// Whether the distributed hash table is enabled for peer discovery.
    pub enable_dht: bool,
    /// DHT bootstrap nodes (host:port).
    pub dht_bootstrap_nodes: Vec<String>,
    /// DHT router endpoints (host:port).
    pub dht_router_nodes: Vec<String>,
    /// Whether local service discovery is enabled.
    pub enable_lsd: Toggle,
    /// Whether `UPnP` port mapping is enabled.
    pub enable_upnp: Toggle,
    /// Whether NAT-PMP port mapping is enabled.
    pub enable_natpmp: Toggle,
    /// Whether peer exchange (PEX) is enabled.
    pub enable_pex: Toggle,
    /// Optional outgoing port range for peer connections.
    pub outgoing_ports: Option<OutgoingPortRange>,
    /// Optional DSCP/TOS codepoint (0-63) for peer sockets.
    pub peer_dscp: Option<u8>,
    /// Optional global peer connection limit.
    pub connections_limit: Option<i32>,
    /// Optional per-torrent peer connection limit.
    pub connections_limit_per_torrent: Option<i32>,
    /// Optional unchoke slot limit.
    pub unchoke_slots: Option<i32>,
    /// Optional half-open connection limit.
    pub half_open_limit: Option<i32>,
    /// Whether anonymous mode is enabled.
    pub anonymous_mode: Toggle,
    /// Whether peers must be proxied.
    pub force_proxy: Toggle,
    /// Whether RC4 encryption should be preferred.
    pub prefer_rc4: Toggle,
    /// Whether multiple connections per IP are allowed.
    pub allow_multiple_connections_per_ip: Toggle,
    /// Whether outgoing uTP connections are enabled.
    pub enable_outgoing_utp: Toggle,
    /// Whether incoming uTP connections are enabled.
    pub enable_incoming_utp: Toggle,
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
    /// IP filter and optional remote blocklist configuration.
    pub ip_filter: Option<IpFilterRuntimeConfig>,
}

/// IPv6 preference policy applied at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ipv6Mode {
    /// Disable IPv6 listeners and prefer IPv4.
    #[default]
    Disabled,
    /// Enable IPv6 alongside IPv4.
    Enabled,
    /// Prefer IPv6 addresses while keeping IPv4 listeners.
    PreferV6,
}

impl Ipv6Mode {
    #[must_use]
    /// Convert the policy to a compact numeric representation.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Enabled => 1,
            Self::PreferV6 => 2,
        }
    }
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

/// Inclusive IP range used for filtering peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpFilterRule {
    /// Start address of the blocked range.
    pub start: String,
    /// End address of the blocked range.
    pub end: String,
}

/// IP filter configuration carried to the runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IpFilterRuntimeConfig {
    /// Aggregated blocked ranges from inline CIDRs and remote blocklists.
    pub rules: Vec<IpFilterRule>,
    /// Optional remote blocklist URL (for observability/caching).
    pub blocklist_url: Option<String>,
    /// Cached `ETag` from the last successful fetch.
    pub etag: Option<String>,
    /// Timestamp of the last successful refresh.
    pub last_updated_at: Option<String>,
}

/// Outgoing port range applied to peer connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutgoingPortRange {
    /// Start port, inclusive.
    pub start: u16,
    /// End port, inclusive.
    pub end: u16,
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
