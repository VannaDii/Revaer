//! Manual indexer search feature wiring.
//!
//! # Design
//! - Keep manual search request shaping in pure helpers so the form remains testable.
//! - Route all transport calls through a feature-local API shim backed by shared services.
//! - Restrict mutable UI concerns to the view while reusing API DTOs directly.

#[cfg(target_arch = "wasm32")]
pub mod api;
pub mod logic;
pub mod state;
#[cfg(target_arch = "wasm32")]
pub mod view;
