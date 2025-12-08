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

//! Shared test helpers used across integration suites.
//! Layout: fixtures.rs (env/helpers), postgres.rs (docker-backed fixtures), mocks.rs (fake clients), assert.rs (test assertions).

pub mod assert;
pub mod fixtures;
pub mod mocks;
pub mod postgres;
