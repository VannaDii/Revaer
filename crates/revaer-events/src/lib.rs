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
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::multiple_crate_versions)]
#![allow(unexpected_cfgs)]

//! Core event bus for the Revaer platform.
//! Layout: `topics.rs` (identifiers), `payloads.rs` (event types), `routing.rs` (bus helpers).

pub mod payloads;
pub mod routing;
pub mod topics;

pub use payloads::{
    DEFAULT_REPLAY_CAPACITY, DiscoveredFile, Event, EventEnvelope, EventId, TorrentState,
};
pub use routing::{EventBus, EventStream};
