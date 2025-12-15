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

#[cfg(test)]
mod tests {
    use super::ffi;
    use std::mem;

    #[test]
    fn engine_option_layout_is_stable() {
        assert_eq!(mem::size_of::<ffi::EngineNetworkOptions>(), 152);
        assert_eq!(mem::size_of::<ffi::EngineLimitOptions>(), 72);
        assert_eq!(mem::size_of::<ffi::EngineStorageOptions>(), 48);
        assert_eq!(mem::size_of::<ffi::EngineBehaviorOptions>(), 1);
        assert_eq!(mem::size_of::<ffi::TrackerProxyOptions>(), 80);
        assert_eq!(mem::size_of::<ffi::EngineTrackerOptions>(), 248);
        assert_eq!(mem::size_of::<ffi::EngineOptions>(), 528);
    }
}
