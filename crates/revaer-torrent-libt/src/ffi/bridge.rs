#[cxx::bridge(namespace = "revaer")]
/// Native bridge types and functions exposed to Rust.
pub mod ffi {
    /// Options used when constructing a libtorrent session.
    #[derive(Debug)]
    struct SessionOptions {
        /// Root path for active downloads.
        download_root: String,
        /// Directory used to persist resume data.
        resume_dir: String,
        /// Whether to enable DHT.
        enable_dht: bool,
        /// Default sequential download preference.
        sequential_default: bool,
    }

    /// Network-centric options applied to the session.
    #[derive(Debug)]
    struct EngineNetworkOptions {
        /// Desired listen port for the engine.
        listen_port: i32,
        /// Whether to apply the listen port override.
        set_listen_port: bool,
        /// Explicit listen interfaces (host/device/IP + port).
        listen_interfaces: Vec<String>,
        /// Whether explicit listen interfaces were provided.
        has_listen_interfaces: bool,
        /// Whether to enable DHT.
        enable_dht: bool,
        /// Whether to enable local service discovery.
        enable_lsd: bool,
        /// Whether to enable `UPnP` port mappings.
        enable_upnp: bool,
        /// Whether to enable NAT-PMP port mappings.
        enable_natpmp: bool,
        /// Whether to enable peer exchange/uTP.
        enable_pex: bool,
        /// Optional starting port for outgoing connections.
        outgoing_port_min: i32,
        /// Optional ending port for outgoing connections.
        outgoing_port_max: i32,
        /// Whether a port range override is present.
        has_outgoing_port_range: bool,
        /// Optional DSCP/TOS value for peer sockets.
        peer_dscp: i32,
        /// Whether a DSCP/TOS value was provided.
        has_peer_dscp: bool,
        /// Whether anonymous mode is enabled.
        anonymous_mode: bool,
        /// Whether peers must be proxied.
        force_proxy: bool,
        /// Whether RC4 encryption should be preferred.
        prefer_rc4: bool,
        /// Whether multiple connections per IP are allowed.
        allow_multiple_connections_per_ip: bool,
        /// Whether outgoing uTP is enabled.
        enable_outgoing_utp: bool,
        /// Whether incoming uTP is enabled.
        enable_incoming_utp: bool,
        /// DHT bootstrap nodes (host:port).
        dht_bootstrap_nodes: Vec<String>,
        /// DHT router endpoints (host:port).
        dht_router_nodes: Vec<String>,
        /// Encryption policy flag.
        encryption_policy: u8,
        /// IP filter rules (inclusive start/end).
        ip_filter_rules: Vec<IpFilterRule>,
        /// Whether an IP filter should be applied.
        has_ip_filter: bool,
    }

    /// Inclusive IP filter rule.
    #[derive(Debug)]
    struct IpFilterRule {
        /// Start address of the blocked range.
        start: String,
        /// End address of the blocked range.
        end: String,
    }

    /// Throughput and concurrency limits for the session.
    #[derive(Debug)]
    struct EngineLimitOptions {
        /// Maximum active torrents.
        max_active: i32,
        /// Global download cap in bytes per second.
        download_rate_limit: i64,
        /// Global upload cap in bytes per second.
        upload_rate_limit: i64,
        /// Optional global peer connection limit.
        connections_limit: i32,
        /// Optional per-torrent peer connection limit.
        connections_limit_per_torrent: i32,
        /// Optional unchoke slot limit.
        unchoke_slots: i32,
        /// Optional half-open connection limit.
        half_open_limit: i32,
    }

    /// Storage paths and behaviour applied to the session.
    #[derive(Debug)]
    struct EngineStorageOptions {
        /// Root path for downloads.
        download_root: String,
        /// Directory for resume data.
        resume_dir: String,
    }

    /// Behavioural defaults applied to new torrents.
    #[derive(Debug)]
    struct EngineBehaviorOptions {
        /// Default sequential preference for torrents.
        sequential_default: bool,
    }

    /// Proxy configuration for tracker announces.
    #[derive(Debug)]
    struct TrackerProxyOptions {
        /// Proxy host.
        host: String,
        /// Secret reference for proxy username when provided.
        username_secret: String,
        /// Secret reference for proxy password when provided.
        password_secret: String,
        /// Proxy port.
        port: u16,
        /// Proxy type (http/https/socks5).
        kind: u8,
        /// Whether peer connections should also use the proxy.
        proxy_peers: bool,
        /// Flag indicating whether a proxy configuration was supplied.
        has_proxy: bool,
    }

    /// Tracker-related options for the session.
    #[derive(Debug)]
    struct EngineTrackerOptions {
        /// Default trackers applied to every torrent.
        default_trackers: Vec<String>,
        /// Extra trackers appended to defaults.
        extra_trackers: Vec<String>,
        /// Whether request trackers should replace defaults.
        replace_trackers: bool,
        /// Custom tracker user-agent.
        user_agent: String,
        /// Flag indicating whether user-agent was set.
        has_user_agent: bool,
        /// Announce IP override.
        announce_ip: String,
        /// Flag indicating whether announce IP was set.
        has_announce_ip: bool,
        /// Listen interface override for tracker announces.
        listen_interface: String,
        /// Flag indicating whether listen interface was set.
        has_listen_interface: bool,
        /// Tracker request timeout in milliseconds.
        request_timeout_ms: i64,
        /// Flag indicating whether timeout was set.
        has_request_timeout: bool,
        /// Whether to announce to all trackers.
        announce_to_all: bool,
        /// Proxy configuration for tracker announces.
        proxy: TrackerProxyOptions,
    }

    /// Runtime engine configuration forwarded to the native layer.
    #[derive(Debug)]
    struct EngineOptions {
        /// Network-facing options (listen, DHT, encryption).
        network: EngineNetworkOptions,
        /// Throughput and concurrency limits.
        limits: EngineLimitOptions,
        /// Storage configuration.
        storage: EngineStorageOptions,
        /// Behavioural defaults.
        behavior: EngineBehaviorOptions,
        /// Tracker settings.
        tracker: EngineTrackerOptions,
    }

    /// Request payload for adding a torrent to the native session.
    #[derive(Debug)]
    struct AddTorrentRequest {
        /// Torrent identifier.
        id: String,
        /// Whether the source is magnet or metainfo.
        source_kind: SourceKind,
        /// Magnet URI when applicable.
        magnet_uri: String,
        /// Raw metainfo payload when applicable.
        metainfo: Vec<u8>,
        /// Override download directory.
        download_dir: String,
        /// Flag indicating whether a download dir override was provided.
        has_download_dir: bool,
        /// Sequential preference override.
        sequential: bool,
        /// Flag indicating whether sequential override was provided.
        has_sequential_override: bool,
        /// Whether the torrent should start paused.
        start_paused: bool,
        /// Flag indicating whether a paused override was provided.
        has_start_paused: bool,
        /// Optional per-torrent peer connection limit.
        max_connections: i32,
        /// Flag indicating whether a per-torrent limit was supplied.
        has_max_connections: bool,
        /// Tags associated with the torrent.
        tags: Vec<String>,
        /// Additional trackers provided for this torrent.
        trackers: Vec<String>,
        /// Whether request trackers replace session defaults.
        replace_trackers: bool,
    }

    /// Rate limit update applied globally or for a specific torrent.
    #[derive(Debug)]
    struct LimitRequest {
        /// Whether the limit applies globally.
        apply_globally: bool,
        /// Torrent identifier (empty when global).
        id: String,
        /// Download cap in bytes per second.
        download_bps: i64,
        /// Upload cap in bytes per second.
        upload_bps: i64,
    }

    /// Per-file priority override entry.
    #[derive(Debug)]
    struct FilePriorityOverride {
        /// File index within the torrent.
        index: u32,
        /// Desired priority flag.
        priority: u8,
    }

    /// Selection rules pushed down to libtorrent.
    #[derive(Debug)]
    struct SelectionRules {
        /// Torrent identifier.
        id: String,
        /// Inclusion globs.
        include: Vec<String>,
        /// Exclusion globs.
        exclude: Vec<String>,
        /// Priority overrides.
        priorities: Vec<FilePriorityOverride>,
        /// Whether to drop fluff files.
        skip_fluff: bool,
    }

    /// File metadata emitted by the native session.
    #[derive(Debug)]
    struct NativeFile {
        /// File index within the torrent.
        index: u32,
        /// File path.
        path: String,
        /// File size in bytes.
        size_bytes: u64,
    }

    /// Event envelope emitted by the native session.
    #[derive(Debug)]
    struct NativeEvent {
        /// Torrent identifier.
        id: String,
        /// Event kind.
        kind: NativeEventKind,
        /// Torrent state snapshot.
        state: NativeTorrentState,
        /// Torrent name.
        name: String,
        /// Download directory in use.
        download_dir: String,
        /// Library path once moved.
        library_path: String,
        /// Bytes downloaded so far.
        bytes_downloaded: u64,
        /// Total bytes expected.
        bytes_total: u64,
        /// Current download rate.
        download_bps: u64,
        /// Current upload rate.
        upload_bps: u64,
        /// Current share ratio.
        ratio: f64,
        /// File list snapshot.
        files: Vec<NativeFile>,
        /// Serialized resume data.
        resume_data: Vec<u8>,
        /// Human-readable message, when present.
        message: String,
    }

    /// Event kinds surfaced by the native bridge.
    #[derive(Debug)]
    enum NativeEventKind {
        /// Newly discovered files.
        FilesDiscovered,
        /// Progress update.
        Progress,
        /// State transition.
        StateChanged,
        /// Completion notification.
        Completed,
        /// Metadata update.
        MetadataUpdated,
        /// New resume data generated.
        ResumeData,
        /// Error encountered.
        Error,
    }

    /// Torrent lifecycle states emitted by libtorrent.
    #[derive(Debug)]
    enum NativeTorrentState {
        /// Waiting to start.
        Queued,
        /// Fetching metadata.
        FetchingMetadata,
        /// Actively downloading.
        Downloading,
        /// Seeding.
        Seeding,
        /// Completed state.
        Completed,
        /// Failed state.
        Failed,
        /// Explicitly stopped.
        Stopped,
    }

    /// Source type used when adding a torrent.
    #[derive(Debug)]
    enum SourceKind {
        /// Magnet URI source.
        Magnet,
        /// Metainfo bytes source.
        Metainfo,
    }

    unsafe extern "C++" {
        include!("revaer/session.hpp");

        /// Opaque handle to the native libtorrent session.
        type Session;

        /// Create a new libtorrent session with the provided options.
        #[must_use]
        fn new_session(options: &SessionOptions) -> UniquePtr<Session>;
        /// Apply an engine profile to the running session.
        #[must_use]
        fn apply_engine_profile(self: Pin<&mut Session>, options: &EngineOptions) -> String;
        /// Add a torrent to the session.
        #[must_use]
        fn add_torrent(self: Pin<&mut Session>, request: &AddTorrentRequest) -> String;
        /// Remove a torrent and optionally its data.
        #[must_use]
        fn remove_torrent(self: Pin<&mut Session>, id: &str, with_data: bool) -> String;
        /// Pause a torrent in the session.
        #[must_use]
        fn pause_torrent(self: Pin<&mut Session>, id: &str) -> String;
        /// Resume a paused torrent.
        #[must_use]
        fn resume_torrent(self: Pin<&mut Session>, id: &str) -> String;
        /// Toggle sequential mode for a torrent.
        #[must_use]
        fn set_sequential(self: Pin<&mut Session>, id: &str, sequential: bool) -> String;
        /// Load fast-resume payload for a torrent.
        #[must_use]
        fn load_fastresume(self: Pin<&mut Session>, id: &str, payload: &[u8]) -> String;
        /// Apply rate limits to the session or a specific torrent.
        #[must_use]
        fn update_limits(self: Pin<&mut Session>, request: &LimitRequest) -> String;
        /// Update selection rules for a torrent.
        #[must_use]
        fn update_selection(self: Pin<&mut Session>, request: &SelectionRules) -> String;
        /// Trigger tracker reannounce.
        #[must_use]
        fn reannounce(self: Pin<&mut Session>, id: &str) -> String;
        /// Recheck on-disk data for a torrent.
        #[must_use]
        fn recheck(self: Pin<&mut Session>, id: &str) -> String;
        /// Poll pending events from the session.
        #[must_use]
        fn poll_events(self: Pin<&mut Session>) -> Vec<NativeEvent>;
    }
}
