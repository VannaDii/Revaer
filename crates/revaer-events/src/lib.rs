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

//! Core event bus for the Revaer platform.
//! Layout: `topics.rs` (identifiers), `payloads.rs` (event types), `routing.rs` (bus helpers).

pub mod payloads;
#[cfg(not(target_arch = "wasm32"))]
pub mod routing;
pub mod topics;

pub use payloads::{
    DEFAULT_REPLAY_CAPACITY, DiscoveredFile, Event, EventEnvelope, EventId, TorrentState,
};
#[cfg(not(target_arch = "wasm32"))]
pub use routing::{EventBus, EventStream};
