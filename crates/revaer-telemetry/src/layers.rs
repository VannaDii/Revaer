//! Request ID middleware helpers for Tower-compatible stacks.
//!
//! # Design
//! - Provides dedicated layers for generating and propagating `x-request-id`.
//! - Keeps layer construction separate from logging initialisation to simplify wiring.

use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

/// Factory for the `x-request-id` generator layer.
#[must_use]
pub fn set_request_id_layer() -> SetRequestIdLayer<MakeRequestUuid> {
    SetRequestIdLayer::x_request_id(MakeRequestUuid)
}

/// Layer that propagates an incoming `x-request-id` header.
#[must_use]
pub fn propagate_request_id_layer() -> PropagateRequestIdLayer {
    PropagateRequestIdLayer::x_request_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_id_layers_can_be_constructed() {
        let _set_layer = set_request_id_layer();
        let _prop_layer = propagate_request_id_layer();
    }
}
