#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Binary entrypoint that wires the Revaer services together and launches the
//! async orchestrators.

use revaer_app::{AppError, AppResult, run_app_with_database_url};

/// Bootstraps the Revaer application and blocks until shutdown.
#[tokio::main]
async fn main() -> AppResult<()> {
    run_entrypoint().await
}

async fn run_entrypoint() -> AppResult<()> {
    run_entrypoint_with(std::env::var("DATABASE_URL").ok()).await
}

async fn run_entrypoint_with(database_url: Option<String>) -> AppResult<()> {
    let database_url = database_url.ok_or(AppError::MissingEnv {
        name: "DATABASE_URL",
    })?;
    run_app_with_database_url(database_url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_entrypoint_requires_database_url() {
        let result = run_entrypoint_with(None).await;
        assert!(matches!(
            result,
            Err(AppError::MissingEnv {
                name: "DATABASE_URL"
            })
        ));
    }
}
