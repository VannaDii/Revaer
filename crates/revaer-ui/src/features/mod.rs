//! Vertical feature slices for the Web UI.
#[cfg(any(target_arch = "wasm32", test))]
pub mod dashboard;
#[cfg(any(target_arch = "wasm32", test))]
pub mod health;
#[cfg(any(target_arch = "wasm32", test))]
pub mod logs;
#[cfg(any(target_arch = "wasm32", test))]
pub mod settings;
#[cfg(any(target_arch = "wasm32", test))]
pub mod torrents;
