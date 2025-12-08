//! CXX bridge exposing the native libtorrent session surface.

/// Raw bindings to the libtorrent session exposed via CXX.
#[allow(unsafe_code)]
pub mod bridge;

#[cfg(feature = "libtorrent")]
#[allow(unsafe_code, clippy::non_send_fields_in_send_ty)]
// SAFETY: the C++ session wrapper is created on the main thread and moved into the
// dedicated worker task exactly once; it is never shared concurrently across threads.
unsafe impl Send for bridge::ffi::Session {}

/// Re-export of the generated CXX bindings for consumers within this crate.
pub use bridge::ffi;
