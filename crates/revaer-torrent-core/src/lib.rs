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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Engine-agnostic torrent interfaces and DTOs shared across the workspace.
//! Layout: `model/` (core types, DTOs), `service/` (engine/workflow traits).

pub mod model;
pub mod service;

pub use model::{
    AddTorrent, AddTorrentOptions, EngineEvent, FilePriority, FilePriorityOverride,
    FileSelectionRules, FileSelectionUpdate, PeerChoke, PeerInterest, PeerSnapshot, RemoveTorrent,
    StorageMode, TorrentFile, TorrentProgress, TorrentRateLimit, TorrentRates, TorrentSource,
    TorrentStatus,
};
pub use service::{TorrentEngine, TorrentInspector, TorrentWorkflow};
