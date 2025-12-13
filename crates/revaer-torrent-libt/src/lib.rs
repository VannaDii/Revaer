#![cfg_attr(not(feature = "libtorrent"), forbid(unsafe_code))]
#![cfg_attr(feature = "libtorrent", deny(unsafe_code))]
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

//! Libtorrent adapter implementation backed by the native C++ session bridge.

/// Safe wrapper around the libtorrent worker and FFI bindings.
pub mod adapter;
/// Engine command definitions and shared request types used by the adapter.
pub mod command;
#[cfg(feature = "libtorrent")]
pub mod convert;
#[cfg(feature = "libtorrent")]
pub mod ffi;
/// Session abstraction and native/stub implementations.
pub mod session;
mod store;
/// Strongly typed runtime configuration inputs and policies.
pub mod types;
/// Background worker that drives the libtorrent session.
pub mod worker;

pub use adapter::LibtorrentEngine;
pub use command::EngineCommand;
pub use store::{FastResumeStore, StoredTorrentMetadata, StoredTorrentState};
pub use types::{
    EncryptionPolicy, EngineRuntimeConfig, TrackerProxyRuntime, TrackerProxyType,
    TrackerRuntimeConfig,
};
