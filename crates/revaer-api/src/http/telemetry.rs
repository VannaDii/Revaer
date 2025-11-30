//! HTTP metrics middleware for request counting.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

use crate::http::constants::HEADER_REQUEST_ID;
use axum::extract::MatchedPath;
use axum::http::Request;
use revaer_telemetry::{Metrics, with_request_context};
use tower::{Layer, Service};

/// Wraps HTTP services to record request metrics per route and status code.
#[derive(Clone)]
pub(crate) struct HttpMetricsLayer {
    telemetry: Metrics,
}

impl HttpMetricsLayer {
    /// Construct a new metrics layer with the shared telemetry handle.
    pub(crate) const fn new(telemetry: Metrics) -> Self {
        Self { telemetry }
    }
}

impl<S> Layer<S> for HttpMetricsLayer {
    type Service = HttpMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpMetricsService {
            inner,
            telemetry: self.telemetry.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct HttpMetricsService<S> {
    inner: S,
    telemetry: Metrics,
}

impl<S, B> Service<Request<B>> for HttpMetricsService<S>
where
    S: Service<Request<B>, Response = axum::response::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let route = req.extensions().get::<MatchedPath>().map_or_else(
            || req.uri().path().to_string(),
            |matched| matched.as_str().to_string(),
        );
        let request_id = req
            .headers()
            .get(HEADER_REQUEST_ID)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let telemetry = self.telemetry.clone();
        let fut = self.inner.call(req);

        Box::pin(async move {
            with_request_context(request_id, route.clone(), async move {
                let response = fut.await?;
                telemetry.inc_http_request(&route, response.status().as_u16());
                Ok(response)
            })
            .await
        })
    }
}
