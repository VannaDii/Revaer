//! CXX bridge exposing the native libtorrent session surface.

/// Raw bindings to the libtorrent session exposed via CXX.
#[allow(unsafe_code)]
pub mod bridge;

/// Errors returned when constructing FFI session handles.
#[derive(Debug)]
pub enum SessionHandleError {
    /// The C++ session constructor returned a null pointer.
    NullSession,
}

impl std::fmt::Display for SessionHandleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullSession => formatter.write_str("native session initialization returned null"),
        }
    }
}

impl std::error::Error for SessionHandleError {}

/// Owned handle to the native session pointer.
#[cfg(libtorrent_native)]
#[allow(unsafe_code)]
pub struct SessionHandle {
    inner: *mut bridge::ffi::Session,
}

#[cfg(libtorrent_native)]
#[allow(unsafe_code)]
unsafe impl Send for SessionHandle {}

#[cfg(libtorrent_native)]
#[allow(unsafe_code)]
impl SessionHandle {
    /// Create a new session handle from the C++ constructor.
    ///
    /// # Errors
    ///
    /// Returns `SessionHandleError::NullSession` when the native constructor returns null.
    pub fn new(options: &bridge::ffi::SessionOptions) -> Result<Self, SessionHandleError> {
        let raw = bridge::ffi::new_session(options).into_raw();
        if raw.is_null() {
            return Err(SessionHandleError::NullSession);
        }
        Ok(Self { inner: raw })
    }

    /// Borrow the session mutably, pinned for C++ method calls.
    pub fn pin_mut(&mut self) -> std::pin::Pin<&mut bridge::ffi::Session> {
        unsafe { std::pin::Pin::new_unchecked(&mut *self.inner) }
    }
}

#[cfg(libtorrent_native)]
#[allow(unsafe_code)]
impl AsRef<bridge::ffi::Session> for SessionHandle {
    fn as_ref(&self) -> &bridge::ffi::Session {
        unsafe { &*self.inner }
    }
}

#[cfg(libtorrent_native)]
#[allow(unsafe_code)]
impl Drop for SessionHandle {
    fn drop(&mut self) {
        unsafe {
            drop(cxx::UniquePtr::from_raw(self.inner));
        }
    }
}

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
        assert_eq!(storage, 88, "{sizes}");
        assert_eq!(behavior, 5, "{sizes}");
        assert_eq!(proxy, 128, "{sizes}");
        assert_eq!(tracker, 544, "{sizes}");
        assert_eq!(options, 944, "{sizes}");
    }
}
