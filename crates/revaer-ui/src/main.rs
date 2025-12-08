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
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
//! Revaer UI wasm entry point and native stub fallback.

fn main() {
    #[cfg(target_arch = "wasm32")]
    revaer_ui::run_app();

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::{self, Write};

        let mut stderr = io::stderr().lock();
        if let Err(err) = stderr.write_all(
            b"The revaer-ui binary is intended for wasm32; build with `trunk build` or `cargo build --target wasm32-unknown-unknown`.\n",
        ) {
            panic!("failed to write warning: {err}");
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn native_main_writes_warning() {
        // Ensure the native stub executes without panicking.
        main();
    }
}
