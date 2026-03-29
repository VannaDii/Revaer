//! Indexer administration feature slice.

#[cfg(target_arch = "wasm32")]
pub(crate) mod api;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) mod logic;
#[cfg(target_arch = "wasm32")]
pub(crate) mod state;
#[cfg(target_arch = "wasm32")]
pub(crate) mod view;
