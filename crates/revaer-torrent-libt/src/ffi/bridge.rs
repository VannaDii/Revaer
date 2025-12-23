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
        /// Share ratio threshold before stopping seeding.
        seed_ratio_limit: f64,
        /// Whether a ratio limit was provided.
        has_seed_ratio_limit: bool,
        /// Seeding time limit in seconds.
        seed_time_limit: i64,
        /// Whether a time limit was provided.
        has_seed_time_limit: bool,
        /// Optional global peer connection limit.
        connections_limit: i32,
        /// Optional per-torrent peer connection limit.
        connections_limit_per_torrent: i32,
        /// Optional unchoke slot limit.
        unchoke_slots: i32,
        /// Optional half-open connection limit.
        half_open_limit: i32,
        /// Choking strategy used while downloading.
        choking_algorithm: i32,
        /// Choking strategy used while seeding.
        seed_choking_algorithm: i32,
        /// Whether strict super-seeding is enforced.
        strict_super_seeding: bool,
        /// Optional optimistic unchoke slot override.
        optimistic_unchoke_slots: i32,
        /// Whether an optimistic unchoke override was provided.
        has_optimistic_unchoke_slots: bool,
        /// Optional maximum queued disk bytes override.
        max_queued_disk_bytes: i32,
        /// Whether a queued disk byte limit was provided.
        has_max_queued_disk_bytes: bool,
        /// Stats alert interval in milliseconds.
        stats_interval_ms: i32,
        /// Whether a stats interval override was provided.
        has_stats_interval: bool,
    }

    /// Storage paths and behaviour applied to the session.
    #[derive(Debug)]
    struct EngineStorageOptions {
        /// Root path for downloads.
        download_root: String,
        /// Directory for resume data.
        resume_dir: String,
        /// Storage allocation mode (sparse/allocate).
        storage_mode: i32,
        /// Whether partfiles should be used.
        use_partfile: bool,
        /// Optional disk read mode.
        disk_read_mode: i32,
        /// Whether a disk read mode override was provided.
        has_disk_read_mode: bool,
        /// Optional disk write mode.
        disk_write_mode: i32,
        /// Whether a disk write mode override was provided.
        has_disk_write_mode: bool,
        /// Whether piece hashes should be verified.
        verify_piece_hashes: bool,
        /// Optional cache size in MiB.
        cache_size: i32,
        /// Whether a cache size override was provided.
        has_cache_size: bool,
        /// Optional cache expiry in seconds.
        cache_expiry: i32,
        /// Whether a cache expiry override was provided.
        has_cache_expiry: bool,
        /// Whether disk reads should be coalesced.
        coalesce_reads: bool,
        /// Whether disk writes should be coalesced.
        coalesce_writes: bool,
        /// Whether to use the shared disk cache pool.
        use_disk_cache_pool: bool,
    }

    /// Snapshot of cache-related storage settings in the native session.
    #[derive(Debug)]
    struct EngineStorageState {
        /// Cached block capacity in MiB.
        cache_size: i32,
        /// Cached block expiry in seconds.
        cache_expiry: i32,
        /// Bitfield of storage flags (partfile/coalesce settings).
        flags: u8,
        /// Disk read mode in use.
        disk_read_mode: i32,
        /// Disk write mode in use.
        disk_write_mode: i32,
        /// Whether piece hashes are verified.
        verify_piece_hashes: bool,
    }

    /// Snapshot of peer class configuration applied to the native session.
    #[derive(Debug)]
    struct EnginePeerClassState {
        /// Peer class ids configured in the session.
        configured_ids: Vec<u8>,
        /// Default peer class ids applied to new torrents.
        default_ids: Vec<u8>,
    }

    /// Behavioural defaults applied to new torrents.
    #[derive(Debug)]
    struct EngineBehaviorOptions {
        /// Default sequential preference for torrents.
        sequential_default: bool,
        /// Whether torrents should start as auto-managed by default.
        auto_managed: bool,
        /// Whether queueing should prefer seeds when allocating slots.
        auto_manage_prefer_seeds: bool,
        /// Whether idle torrents are excluded from active slot accounting.
        dont_count_slow_torrents: bool,
        /// Whether torrents should default to super-seeding.
        super_seeding: bool,
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

    /// Authentication material for tracker announces.
    #[derive(Debug)]
    struct TrackerAuthOptions {
        /// Plain username resolved from secrets.
        username: String,
        /// Plain password resolved from secrets.
        password: String,
        /// Cookie or trackerid value applied on announce.
        cookie: String,
        /// Secret reference used for the username when provided.
        username_secret: String,
        /// Secret reference used for the password when provided.
        password_secret: String,
        /// Secret reference used for the cookie when provided.
        cookie_secret: String,
        /// Flag indicating whether username is set.
        has_username: bool,
        /// Flag indicating whether password is set.
        has_password: bool,
        /// Flag indicating whether cookie is set.
        has_cookie: bool,
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
        /// Optional client certificate path for tracker TLS.
        ssl_cert: String,
        /// Flag indicating whether a client certificate was provided.
        has_ssl_cert: bool,
        /// Optional client key path for tracker TLS.
        ssl_private_key: String,
        /// Flag indicating whether a client key was provided.
        has_ssl_private_key: bool,
        /// Optional CA certificate bundle path for tracker TLS.
        ssl_ca_cert: String,
        /// Flag indicating whether a CA bundle was provided.
        has_ssl_ca_cert: bool,
        /// Whether to verify tracker TLS certificates.
        ssl_tracker_verify: bool,
        /// Flag indicating whether tracker verification was provided.
        has_ssl_tracker_verify: bool,
        /// Whether to announce to all trackers.
        announce_to_all: bool,
        /// Proxy configuration for tracker announces.
        proxy: TrackerProxyOptions,
        /// Authentication material for tracker announces.
        auth: TrackerAuthOptions,
    }

    /// Peer class configuration forwarded to the native layer.
    #[derive(Debug)]
    struct PeerClassConfig {
        /// Stable identifier for the class (0-31).
        id: u8,
        /// Human-readable label.
        label: String,
        /// Download bandwidth priority.
        download_priority: u8,
        /// Upload bandwidth priority.
        upload_priority: u8,
        /// Connection limit multiplier.
        connection_limit_factor: u16,
        /// Whether unchoke slots are ignored for this class.
        ignore_unchoke_slots: bool,
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
        /// Peer class definitions.
        peer_classes: Vec<PeerClassConfig>,
        /// Default peer class ids applied to new torrents.
        default_peer_classes: Vec<u8>,
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
        /// Requested storage allocation mode.
        storage_mode: i32,
        /// Flag indicating whether storage mode override was provided.
        has_storage_mode: bool,
        /// Sequential preference override.
        sequential: bool,
        /// Flag indicating whether sequential override was provided.
        has_sequential_override: bool,
        /// Whether the torrent should start paused.
        start_paused: bool,
        /// Flag indicating whether a paused override was provided.
        has_start_paused: bool,
        /// Whether the torrent should be auto-managed by the session.
        auto_managed: bool,
        /// Flag indicating whether auto-managed was explicitly provided.
        has_auto_managed: bool,
        /// Desired queue position when auto-managed is disabled.
        queue_position: i32,
        /// Flag indicating whether queue position was provided.
        has_queue_position: bool,
        /// Whether the torrent should start in seed mode.
        seed_mode: bool,
        /// Flag indicating whether seed mode was explicitly requested.
        has_seed_mode: bool,
        /// Percentage of pieces to hash before honoring seed mode.
        hash_check_sample_pct: u8,
        /// Flag indicating whether a hash sample was requested.
        has_hash_check_sample: bool,
        /// Whether peer exchange is enabled for this torrent.
        pex_enabled: bool,
        /// Flag indicating whether PEX was explicitly overridden.
        has_pex_enabled: bool,
        /// Whether super-seeding is enabled for this torrent.
        super_seeding: bool,
        /// Flag indicating whether super-seeding was explicitly overridden.
        has_super_seeding: bool,
        /// Optional per-torrent peer connection limit.
        max_connections: i32,
        /// Flag indicating whether a per-torrent limit was supplied.
        has_max_connections: bool,
        /// Optional comment override for the torrent metainfo.
        comment: String,
        /// Flag indicating whether a comment override was supplied.
        has_comment: bool,
        /// Optional source override for the torrent metainfo.
        source: String,
        /// Flag indicating whether a source override was supplied.
        has_source: bool,
        /// Optional private flag override for the torrent metainfo.
        private_flag: bool,
        /// Flag indicating whether a private override was supplied.
        has_private: bool,
        /// Tags associated with the torrent.
        tags: Vec<String>,
        /// Additional trackers provided for this torrent.
        trackers: Vec<String>,
        /// Whether request trackers replace session defaults.
        replace_trackers: bool,
        /// Web seeds attached to the torrent.
        web_seeds: Vec<String>,
        /// Whether web seeds should replace existing entries.
        replace_web_seeds: bool,
        /// Optional tracker authentication overrides.
        tracker_auth: TrackerAuthOptions,
    }

    /// Request payload for authoring a new `.torrent` metainfo payload.
    #[derive(Debug)]
    struct CreateTorrentRequest {
        /// Source file or directory path.
        root_path: String,
        /// Trackers embedded in the metainfo.
        trackers: Vec<String>,
        /// Web seeds embedded in the metainfo.
        web_seeds: Vec<String>,
        /// Inclusion globs.
        include: Vec<String>,
        /// Exclusion globs.
        exclude: Vec<String>,
        /// Whether to drop fluff files.
        skip_fluff: bool,
        /// Requested piece length in bytes.
        piece_length: u32,
        /// Whether a piece length override was provided.
        has_piece_length: bool,
        /// Whether the torrent should be marked as private.
        private_flag: bool,
        /// Optional comment.
        comment: String,
        /// Whether a comment was provided.
        has_comment: bool,
        /// Optional source label.
        source: String,
        /// Whether a source label was provided.
        has_source: bool,
    }

    /// File entry produced during torrent authoring.
    #[derive(Debug)]
    struct CreateTorrentFile {
        /// Relative file path inside the torrent.
        path: String,
        /// File size in bytes.
        size_bytes: u64,
    }

    /// Result payload returned by torrent authoring.
    #[derive(Debug)]
    struct CreateTorrentResult {
        /// Bencoded metainfo payload.
        metainfo: Vec<u8>,
        /// Magnet URI derived from the metainfo.
        magnet_uri: String,
        /// Best available info hash string.
        info_hash: String,
        /// Effective piece length in bytes.
        piece_length: u32,
        /// Total payload size in bytes.
        total_size: u64,
        /// Files included in the torrent.
        files: Vec<CreateTorrentFile>,
        /// Warnings generated during authoring.
        warnings: Vec<String>,
        /// Effective tracker list.
        trackers: Vec<String>,
        /// Effective web seed list.
        web_seeds: Vec<String>,
        /// Private flag embedded in the metainfo.
        private_flag: bool,
        /// Comment embedded in the metainfo.
        comment: String,
        /// Source label embedded in the metainfo.
        source: String,
        /// Error message when authoring fails.
        error: String,
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

    /// Request to move torrent storage to a new path.
    #[derive(Debug)]
    struct MoveTorrentRequest {
        /// Torrent identifier.
        id: String,
        /// Destination download directory.
        download_dir: String,
    }

    /// Per-torrent option updates applied after admission.
    #[derive(Debug)]
    struct UpdateOptionsRequest {
        /// Torrent identifier.
        id: String,
        /// Optional per-torrent peer connection limit.
        max_connections: i32,
        /// Whether a per-torrent limit override was supplied.
        has_max_connections: bool,
        /// Whether peer exchange should be enabled.
        pex_enabled: bool,
        /// Flag indicating whether the PEX override was provided.
        has_pex_enabled: bool,
        /// Whether super-seeding is enabled for this torrent.
        super_seeding: bool,
        /// Flag indicating whether super-seeding was explicitly overridden.
        has_super_seeding: bool,
        /// Whether the torrent should be auto-managed by the session.
        auto_managed: bool,
        /// Flag indicating whether auto-managed was explicitly provided.
        has_auto_managed: bool,
        /// Desired queue position when auto-managed is disabled.
        queue_position: i32,
        /// Flag indicating whether queue position was provided.
        has_queue_position: bool,
        /// Optional comment override for the torrent metainfo.
        comment: String,
        /// Flag indicating whether a comment override was supplied.
        has_comment: bool,
        /// Optional source override for the torrent metainfo.
        source: String,
        /// Flag indicating whether a source override was supplied.
        has_source: bool,
        /// Optional private flag override for the torrent metainfo.
        private_flag: bool,
        /// Flag indicating whether a private override was supplied.
        has_private: bool,
    }

    /// Tracker list update for an existing torrent.
    #[derive(Debug)]
    struct UpdateTrackersRequest {
        /// Torrent identifier.
        id: String,
        /// Trackers to apply.
        trackers: Vec<String>,
        /// Whether to replace existing trackers.
        replace: bool,
    }

    /// Web seed update for an existing torrent.
    #[derive(Debug)]
    struct UpdateWebSeedsRequest {
        /// Torrent identifier.
        id: String,
        /// Web seeds to apply.
        web_seeds: Vec<String>,
        /// Whether to replace existing web seeds.
        replace: bool,
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
        /// Tracker status entries (if any).
        tracker_statuses: Vec<NativeTrackerStatus>,
        /// Optional component identifier for session errors.
        component: String,
        /// Optional comment captured from metainfo.
        comment: String,
        /// Optional source label captured from metainfo.
        source: String,
        /// Private flag captured from metainfo.
        private_flag: bool,
        /// Whether a private flag was captured.
        has_private: bool,
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
        /// Tracker status update.
        TrackerUpdate,
        /// Session-level error not tied to a specific torrent.
        SessionError,
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

    /// Tracker status reported by libtorrent alerts.
    #[derive(Debug)]
    struct NativeTrackerStatus {
        /// Tracker URL.
        url: String,
        /// Status string (e.g., error/warning).
        status: String,
        /// Optional message content.
        message: String,
    }

    /// Peer snapshot exported from libtorrent.
    #[derive(Debug)]
    struct NativePeerInfo {
        /// Endpoint in host:port form.
        endpoint: String,
        /// Client identifier.
        client: String,
        /// Progress fraction (0.0-1.0).
        progress: f64,
        /// Current download rate in bytes per second.
        download_rate: i64,
        /// Current upload rate in bytes per second.
        upload_rate: i64,
        /// Whether we are interested in the peer.
        interesting: bool,
        /// Whether the peer is choking us.
        choked: bool,
        /// Whether the peer is interested in us.
        remote_interested: bool,
        /// Whether we are choking the peer.
        remote_choked: bool,
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
        /// Create a new `.torrent` metainfo payload.
        #[must_use]
        fn create_torrent(
            self: Pin<&mut Session>,
            request: &CreateTorrentRequest,
        ) -> CreateTorrentResult;
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
        /// Update per-torrent options after admission.
        #[must_use]
        fn update_options(self: Pin<&mut Session>, request: &UpdateOptionsRequest) -> String;
        /// Update trackers for a torrent.
        #[must_use]
        fn update_trackers(self: Pin<&mut Session>, request: &UpdateTrackersRequest) -> String;
        /// Update web seeds for a torrent.
        #[must_use]
        fn update_web_seeds(self: Pin<&mut Session>, request: &UpdateWebSeedsRequest) -> String;
        /// Move torrent storage to a new download directory.
        #[must_use]
        fn move_torrent(self: Pin<&mut Session>, request: &MoveTorrentRequest) -> String;
        /// Trigger tracker reannounce.
        #[must_use]
        fn reannounce(self: Pin<&mut Session>, id: &str) -> String;
        /// Recheck on-disk data for a torrent.
        #[must_use]
        fn recheck(self: Pin<&mut Session>, id: &str) -> String;
        /// Set or clear a deadline for a piece.
        #[must_use]
        fn set_piece_deadline(
            self: Pin<&mut Session>,
            id: &str,
            piece: u32,
            deadline_ms: i32,
            has_deadline: bool,
        ) -> String;
        /// Inspect cache-related storage settings applied to the session.
        #[must_use]
        fn inspect_storage_state(self: &Session) -> EngineStorageState;
        /// Inspect peer class configuration applied to the session.
        #[must_use]
        fn inspect_peer_class_state(self: &Session) -> EnginePeerClassState;
        /// Poll pending events from the session.
        #[must_use]
        fn poll_events(self: Pin<&mut Session>) -> Vec<NativeEvent>;
        /// Retrieve connected peers for a torrent.
        #[must_use]
        fn list_peers(self: Pin<&mut Session>, id: &str) -> Vec<NativePeerInfo>;
    }
}
