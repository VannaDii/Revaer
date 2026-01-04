#![deny(unsafe_code)]
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
#![allow(clippy::multiple_crate_versions)]

//! Libtorrent adapter implementation backed by the native C++ session bridge.

/// Safe wrapper around the libtorrent worker and FFI bindings.
pub mod adapter;
/// Engine command definitions and shared request types used by the adapter.
pub mod command;
#[cfg(libtorrent_native)]
pub mod convert;
pub mod error;
#[cfg(libtorrent_native)]
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
    ChokingAlgorithm, EncryptionPolicy, EngineRuntimeConfig, IpFilterRule, IpFilterRuntimeConfig,
    Ipv6Mode, SeedChokingAlgorithm, Toggle, TrackerAuthRuntime, TrackerProxyRuntime,
    TrackerProxyType, TrackerRuntimeConfig,
};
