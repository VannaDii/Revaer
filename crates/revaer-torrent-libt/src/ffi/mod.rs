//! CXX bridge exposing the native libtorrent session surface.

#[allow(missing_docs)]
pub mod bridge;

#[cfg(feature = "libtorrent")]
#[allow(unsafe_code)]
#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: the C++ session wrapper is created on the main thread and moved into the
// dedicated worker task exactly once; it is never shared concurrently across threads.
unsafe impl Send for bridge::ffi::Session {}

pub use bridge::ffi;
