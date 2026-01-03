//! HTTP DTOs and API error types.
//!
//! # Design
//! - Centralize request/response shapes for the HTTP boundary.
//! - Keep error types alongside DTOs for consistent ProblemDetails mapping.

pub mod errors;

pub use revaer_api_models::*;
