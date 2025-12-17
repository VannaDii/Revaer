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
        let network = mem::size_of::<ffi::EngineNetworkOptions>();
        let limits = mem::size_of::<ffi::EngineLimitOptions>();
        let storage = mem::size_of::<ffi::EngineStorageOptions>();
        let behavior = mem::size_of::<ffi::EngineBehaviorOptions>();
        let proxy = mem::size_of::<ffi::TrackerProxyOptions>();
        let tracker = mem::size_of::<ffi::EngineTrackerOptions>();
        let options = mem::size_of::<ffi::EngineOptions>();
        let sizes = format!(
            "network={network} limits={limits} storage={storage} behavior={behavior} proxy={proxy} tracker={tracker} options={options}"
        );

        assert_eq!(network, 152, "{sizes}");
        assert_eq!(limits, 104, "{sizes}");
        assert_eq!(storage, 72, "{sizes}");
        assert_eq!(behavior, 5, "{sizes}");
        assert_eq!(proxy, 80, "{sizes}");
        assert_eq!(tracker, 400, "{sizes}");
        assert_eq!(options, 736, "{sizes}");
    }
}
