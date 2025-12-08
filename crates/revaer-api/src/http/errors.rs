//! RFC9457-style API error wrapper.

use std::time::Duration;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[cfg(feature = "compat-qb")]
use crate::http::constants::PROBLEM_FORBIDDEN;
use crate::http::constants::{
    PROBLEM_BAD_REQUEST, PROBLEM_CONFIG_INVALID, PROBLEM_CONFLICT, PROBLEM_INTERNAL,
    PROBLEM_NOT_FOUND, PROBLEM_RATE_LIMITED, PROBLEM_SERVICE_UNAVAILABLE, PROBLEM_SETUP_REQUIRED,
    PROBLEM_UNAUTHORIZED,
};
use crate::http::rate_limit::insert_rate_limit_headers;
use crate::models::{ProblemDetails, ProblemInvalidParam};

/// Structured API error with optional RFC9457 fields.
#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) kind: &'static str,
    title: &'static str,
    detail: Option<String>,
    pub(crate) invalid_params: Option<Vec<ProblemInvalidParam>>,
    pub(crate) rate_limit: Option<ErrorRateLimitContext>,
}

#[derive(Debug)]
pub(crate) struct ErrorRateLimitContext {
    pub(crate) limit: u32,
    pub(crate) remaining: u32,
    pub(crate) retry_after: Option<Duration>,
}

impl ApiError {
    const fn new(status: StatusCode, kind: &'static str, title: &'static str) -> Self {
        Self {
            status,
            kind,
            title,
            detail: None,
            invalid_params: None,
            rate_limit: None,
        }
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub(crate) fn with_invalid_params(mut self, params: Vec<ProblemInvalidParam>) -> Self {
        self.invalid_params = Some(params);
        self
    }

    pub(crate) const fn with_rate_limit_headers(
        mut self,
        limit: u32,
        remaining: u32,
        retry_after: Option<Duration>,
    ) -> Self {
        self.rate_limit = Some(ErrorRateLimitContext {
            limit,
            remaining,
            retry_after,
        });
        self
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PROBLEM_INTERNAL,
            "internal server error",
        )
        .with_detail(message)
    }

    pub(crate) fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            PROBLEM_UNAUTHORIZED,
            "authentication required",
        )
        .with_detail(detail)
    }

    #[cfg(feature = "compat-qb")]
    pub(crate) fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, PROBLEM_FORBIDDEN, "forbidden").with_detail(detail)
    }

    pub(crate) fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, PROBLEM_BAD_REQUEST, "bad request").with_detail(detail)
    }

    pub(crate) fn not_found(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            PROBLEM_NOT_FOUND,
            "resource not found",
        )
        .with_detail(detail)
    }

    pub(crate) fn conflict(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, PROBLEM_CONFLICT, "conflict").with_detail(detail)
    }

    pub(crate) fn setup_required(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            PROBLEM_SETUP_REQUIRED,
            "setup required",
        )
        .with_detail(detail)
    }

    pub(crate) fn config_invalid(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            PROBLEM_CONFIG_INVALID,
            "configuration invalid",
        )
        .with_detail(detail)
    }

    pub(crate) fn service_unavailable(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            PROBLEM_SERVICE_UNAVAILABLE,
            "service unavailable",
        )
        .with_detail(detail)
    }

    pub(crate) fn too_many_requests(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            PROBLEM_RATE_LIMITED,
            "rate limit exceeded",
        )
        .with_detail(detail)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ProblemDetails {
            kind: self.kind.to_string(),
            title: self.title.to_string(),
            status: self.status.as_u16(),
            detail: self.detail,
            invalid_params: self.invalid_params,
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Some(rate) = self.rate_limit {
            insert_rate_limit_headers(
                response.headers_mut(),
                rate.limit,
                rate.remaining,
                rate.retry_after,
            );
        }
        response
    }
}
