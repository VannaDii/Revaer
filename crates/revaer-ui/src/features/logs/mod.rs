//! Logs feature slice.
//!
//! # Design
//! - Keep log streaming concerns in the view module.
//! - Render an append-only log view for operators.

#[cfg(target_arch = "wasm32")]
pub mod view;
