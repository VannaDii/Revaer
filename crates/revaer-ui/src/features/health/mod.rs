//! Health feature slice.
//!
//! # Design
//! - Read health snapshots from the `AppStore` only.
//! - Keep rendering logic in the view module.

#[cfg(target_arch = "wasm32")]
pub mod view;
