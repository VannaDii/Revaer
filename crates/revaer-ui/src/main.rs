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
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]
//! Revaer UI wasm entry point and native stub fallback.

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), std::io::Error> {
    revaer_ui::run_app();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), std::io::Error> {
    use std::io::{self, Write};

    let mut stderr = io::stderr().lock();
    stderr.write_all(
        b"The revaer-ui binary is intended for wasm32; build with `trunk build` or `cargo build --target wasm32-unknown-unknown`.\n",
    )?;
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn native_main_writes_warning() -> std::io::Result<()> {
        // Ensure the native stub executes without panicking.
        main()
    }
}
