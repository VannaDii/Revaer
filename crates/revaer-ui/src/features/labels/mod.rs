//! Label policy feature wiring.
//!
//! # Design
//! - Keep label policy editing contained in a single feature slice.
//! - Restrict API calls to this feature layer to honor UI boundaries.
//! - Surface only persisted policy fields (no phantom settings).

pub mod actions;
#[cfg(target_arch = "wasm32")]
pub mod api;
pub mod logic;
pub mod state;
#[cfg(target_arch = "wasm32")]
pub mod view;
