//! API client context for sharing a singleton client instance.
//!
//! # Design
//! - Create exactly one API client per app boot.
//! - Update auth state via interior mutability to avoid rebuilding clients.

use crate::services::api::ApiClient;
use std::rc::Rc;

/// Shared API client context for UI services.
#[derive(Clone)]
pub(crate) struct ApiCtx {
    /// Singleton API client instance.
    pub client: Rc<ApiClient>,
}

impl ApiCtx {
    /// Create a new context with the configured base URL.
    pub(crate) fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Rc::new(ApiClient::new(base_url)),
        }
    }
}

impl PartialEq for ApiCtx {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.client, &other.client)
    }
}
