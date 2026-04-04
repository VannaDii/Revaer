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

//! Thin entrypoint that delegates to the library for CLI execution.

/// Parses CLI arguments and executes the requested command.
#[tokio::main]
async fn main() {
    let exit_code = revaer_cli::run().await;
    if let Some(exit_code) = exit_code_for_failure(exit_code) {
        std::process::exit(exit_code);
    }
}

fn exit_code_for_failure(exit_code: i32) -> Option<i32> {
    (exit_code != 0).then_some(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_for_failure_ignores_success() {
        assert_eq!(exit_code_for_failure(0), None);
    }

    #[test]
    fn exit_code_for_failure_returns_non_zero_codes() {
        assert_eq!(exit_code_for_failure(2), Some(2));
        assert_eq!(exit_code_for_failure(64), Some(64));
    }
}
