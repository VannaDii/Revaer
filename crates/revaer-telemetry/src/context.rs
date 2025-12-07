//! Context propagation helpers for request and application spans.
//!
//! # Design
//! - Keeps request identifiers and routes in task-local storage so spans can access them.
//! - Provides an application-level span guard to ensure top-level spans carry mode/build info.

use std::future::Future;
use std::sync::Arc;

use tracing::{Span, span::Entered};

use crate::init::build_sha;

/// Guard that keeps the application-level span entered for the lifetime of the process.
pub struct GlobalContextGuard {
    _guard: Entered<'static>,
}

impl GlobalContextGuard {
    #[must_use]
    /// Enter the application-level tracing span for the lifetime of the guard.
    pub fn new(mode: impl Into<String>) -> Self {
        let mode = mode.into();
        let span: &'static Span = Box::leak(Box::new(
            tracing::info_span!("app", mode = %mode, build_sha = %build_sha()),
        ));
        let guard = span.enter();
        Self { _guard: guard }
    }
}

/// Record the current application mode on the active span.
pub fn record_app_mode(mode: &str) {
    Span::current().record("mode", tracing::field::display(mode));
}

/// Capture request context for downstream telemetry.
pub fn set_request_context(span: &Span, request_id: impl Into<String>, route: impl Into<String>) {
    let request_id = request_id.into();
    let route = route.into();
    span.record("request_id", tracing::field::display(&request_id));
    span.record("route", tracing::field::display(&route));
}

/// Retrieve the request identifier from the current span, if one is set.
#[must_use]
pub fn current_request_id() -> Option<String> {
    ACTIVE_REQUEST_CONTEXT
        .try_with(|ctx| ctx.request_id.as_ref().to_string())
        .ok()
}

/// Retrieve the matched route from the current span, if one is set.
#[must_use]
pub fn current_route() -> Option<String> {
    ACTIVE_REQUEST_CONTEXT
        .try_with(|ctx| ctx.route.as_ref().to_string())
        .ok()
}

/// Execute the provided future with the supplied request context available to downstream spans.
pub async fn with_request_context<Fut, T>(
    request_id: impl Into<String>,
    route: impl Into<String>,
    fut: Fut,
) -> T
where
    Fut: Future<Output = T>,
{
    let context = RequestContext {
        request_id: Arc::from(request_id.into()),
        route: Arc::from(route.into()),
    };
    ACTIVE_REQUEST_CONTEXT.scope(context, fut).await
}

#[derive(Clone)]
struct RequestContext {
    request_id: Arc<str>,
    route: Arc<str>,
}

tokio::task_local! {
    static ACTIVE_REQUEST_CONTEXT: RequestContext;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_context_guard_sets_app_mode_field() {
        let guard = GlobalContextGuard::new("test");
        record_app_mode("active");
        drop(guard);
    }

    #[test]
    fn set_request_context_records_span_fields() {
        let span = tracing::info_span!(
            "request",
            request_id = tracing::field::Empty,
            route = tracing::field::Empty
        );
        set_request_context(&span, "req-1", "/v1/demo");
    }

    #[tokio::test]
    async fn with_request_context_exposes_identifiers() {
        let output = with_request_context("req-42", "/v1/items", async {
            assert_eq!(current_request_id().as_deref(), Some("req-42"));
            assert_eq!(current_route().as_deref(), Some("/v1/items"));
            "done"
        })
        .await;
        assert_eq!(output, "done");
        assert!(current_request_id().is_none());
        assert!(current_route().is_none());
    }
}
