//! Logs feature slice.
//!
//! # Design
//! - Keep log processing in testable logic helpers.
//! - Render an append-only log view for operators.

#[cfg(target_arch = "wasm32")]
pub mod view;

pub mod ansi;

pub mod logic;
