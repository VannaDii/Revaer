//! Axum-based HTTP surface area for Revaer.

use anyhow::Result;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

pub struct ApiServer {
    router: Router,
}


impl Default for ApiServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiServer {
    pub fn new() -> Self {
        let router = Router::new().route("/health", get(Self::health));
        Self { router }
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn serve(self, addr: SocketAddr) -> Result<()> {
        info!("Starting API on {}", addr);
        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, self.router.into_make_service()).await?;
        Ok(())
    }

    #[allow(clippy::unused_async)]
    async fn health() -> &'static str {
        "ok"
    }
}
