#[allow(missing_docs)]
#[cxx::bridge(namespace = "revaer")]
pub mod ffi {
    #[derive(Debug)]
    struct SessionOptions {
        download_root: String,
        resume_dir: String,
        enable_dht: bool,
        sequential_default: bool,
    }

    #[derive(Debug)]
    struct EngineOptions {
        listen_port: i32,
        set_listen_port: bool,
        enable_dht: bool,
        max_active: i32,
        download_rate_limit: i64,
        upload_rate_limit: i64,
        sequential_default: bool,
        encryption_policy: u8,
        download_root: String,
        resume_dir: String,
    }

    #[derive(Debug)]
    struct AddTorrentRequest {
        id: String,
        source_kind: SourceKind,
        magnet_uri: String,
        metainfo: Vec<u8>,
        download_dir: String,
        has_download_dir: bool,
        sequential: bool,
        has_sequential_override: bool,
        tags: Vec<String>,
    }

    #[derive(Debug)]
    struct LimitRequest {
        apply_globally: bool,
        id: String,
        download_bps: i64,
        upload_bps: i64,
    }

    #[derive(Debug)]
    struct FilePriorityOverride {
        index: u32,
        priority: u8,
    }

    #[derive(Debug)]
    struct SelectionRules {
        id: String,
        include: Vec<String>,
        exclude: Vec<String>,
        priorities: Vec<FilePriorityOverride>,
        skip_fluff: bool,
    }

    #[derive(Debug)]
    struct NativeFile {
        index: u32,
        path: String,
        size_bytes: u64,
    }

    #[derive(Debug)]
    struct NativeEvent {
        id: String,
        kind: NativeEventKind,
        state: NativeTorrentState,
        name: String,
        download_dir: String,
        library_path: String,
        bytes_downloaded: u64,
        bytes_total: u64,
        download_bps: u64,
        upload_bps: u64,
        ratio: f64,
        files: Vec<NativeFile>,
        resume_data: Vec<u8>,
        message: String,
    }

    #[derive(Debug)]
    enum NativeEventKind {
        FilesDiscovered,
        Progress,
        StateChanged,
        Completed,
        MetadataUpdated,
        ResumeData,
        Error,
    }

    #[derive(Debug)]
    enum NativeTorrentState {
        Queued,
        FetchingMetadata,
        Downloading,
        Seeding,
        Completed,
        Failed,
        Stopped,
    }

    #[derive(Debug)]
    enum SourceKind {
        Magnet,
        Metainfo,
    }

    unsafe extern "C++" {
        include!("revaer/session.hpp");

        type Session;

        #[must_use]
        fn new_session(options: &SessionOptions) -> UniquePtr<Session>;
        #[must_use]
        fn apply_engine_profile(self: Pin<&mut Session>, options: &EngineOptions) -> String;
        #[must_use]
        fn add_torrent(self: Pin<&mut Session>, request: &AddTorrentRequest) -> String;
        #[must_use]
        fn remove_torrent(self: Pin<&mut Session>, id: &str, with_data: bool) -> String;
        #[must_use]
        fn pause_torrent(self: Pin<&mut Session>, id: &str) -> String;
        #[must_use]
        fn resume_torrent(self: Pin<&mut Session>, id: &str) -> String;
        #[must_use]
        fn set_sequential(self: Pin<&mut Session>, id: &str, sequential: bool) -> String;
        #[must_use]
        fn load_fastresume(self: Pin<&mut Session>, id: &str, payload: &[u8]) -> String;
        #[must_use]
        fn update_limits(self: Pin<&mut Session>, request: &LimitRequest) -> String;
        #[must_use]
        fn update_selection(self: Pin<&mut Session>, request: &SelectionRules) -> String;
        #[must_use]
        fn reannounce(self: Pin<&mut Session>, id: &str) -> String;
        #[must_use]
        fn recheck(self: Pin<&mut Session>, id: &str) -> String;
        #[must_use]
        fn poll_events(self: Pin<&mut Session>) -> Vec<NativeEvent>;
    }
}
