//! Core, DOM-free primitives and helpers for the Web UI.
pub mod auth;
pub mod breakpoints;
pub mod events;
#[cfg(any(target_arch = "wasm32", test))]
pub mod logic;
#[cfg(any(target_arch = "wasm32", test))]
pub mod store;
pub mod theme;
pub mod ui;
