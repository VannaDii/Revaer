//! Authentication primitives shared across the UI.
//!
//! # Design
//! - Keep auth state as simple data so callers can store/clear it without side effects.
//! - Treat empty credentials as unauthenticated at the call site.
//! - Leave header encoding to transport clients to keep core DOM-free.

/// Supported authentication modes for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    /// API key header authentication.
    ApiKey,
    /// Local username/password authentication.
    Local,
}

/// Local username/password credential pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalAuth {
    /// Local username value.
    pub username: String,
    /// Local password value.
    pub password: String,
}

/// Active authentication state for outbound requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthState {
    /// API key-based authentication.
    ApiKey(String),
    /// Local basic authentication.
    Local(LocalAuth),
    /// Explicit anonymous access (no headers).
    Anonymous,
}

impl AuthState {
    #[must_use]
    /// Whether this auth state contains usable credentials.
    pub fn has_credentials(&self) -> bool {
        match self {
            Self::ApiKey(key) => !key.trim().is_empty(),
            Self::Local(auth) => {
                !auth.username.trim().is_empty() && !auth.password.trim().is_empty()
            }
            Self::Anonymous => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthState, LocalAuth};

    #[test]
    fn auth_state_api_key_requires_non_empty() {
        assert!(!AuthState::ApiKey(String::new()).has_credentials());
        assert!(!AuthState::ApiKey("   ".to_string()).has_credentials());
        assert!(AuthState::ApiKey("demo:secret".to_string()).has_credentials());
    }

    #[test]
    fn auth_state_local_requires_user_and_pass() {
        let empty = LocalAuth {
            username: String::new(),
            password: String::new(),
        };
        assert!(!AuthState::Local(empty).has_credentials());
        let missing_user = LocalAuth {
            username: " ".to_string(),
            password: "secret".to_string(),
        };
        assert!(!AuthState::Local(missing_user).has_credentials());
        let missing_pass = LocalAuth {
            username: "admin".to_string(),
            password: " ".to_string(),
        };
        assert!(!AuthState::Local(missing_pass).has_credentials());
        let ok = LocalAuth {
            username: "admin".to_string(),
            password: "secret".to_string(),
        };
        assert!(AuthState::Local(ok).has_credentials());
    }
}
