#![cfg_attr(not(target_arch = "wasm32"), allow(unused))]

fn main() {
    #[cfg(target_arch = "wasm32")]
    revaer_ui::run_app();

    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!(
            "The revaer-ui binary is intended for wasm32; build with `trunk build` or `cargo build --target wasm32-unknown-unknown`."
        );
    }
}
