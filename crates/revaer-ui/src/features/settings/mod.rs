//! Settings feature slice.
//!
//! # Design
//! - Keep settings rendering in the view module.
//! - Delegate persistence to app-level callbacks.

#[cfg(target_arch = "wasm32")]
pub mod view;
