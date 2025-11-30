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
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::multiple_crate_versions)]
//! Revaer Web UI (Phase 1) scaffolding.
//! This crate holds the Yew front-end entrypoint plus shared tokens and locale metadata.

pub mod core;
pub mod features;
pub mod i18n;
pub mod models;

#[cfg(target_arch = "wasm32")]
pub mod services;

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod components;

#[cfg(target_arch = "wasm32")]
pub use app::run_app;

pub use core::breakpoints;
pub use core::logic;
pub use core::theme;
pub use core::ui::{Density, Pane, UiMode};
pub use features::torrents::actions::TorrentAction;
pub use features::torrents::actions::success_message;
pub use features::torrents::state;

#[cfg(test)]
mod tests {
    use crate::breakpoints::{self, for_width};
    use crate::i18n::{LocaleCode, TranslationBundle};

    #[test]
    fn breakpoint_selection_matches_ranges() {
        assert_eq!(for_width(0).name, breakpoints::XS.name);
        assert_eq!(for_width(480).name, breakpoints::SM.name);
        assert_eq!(for_width(1024).name, breakpoints::LG.name);
        assert_eq!(for_width(2000).name, breakpoints::XXL.name);
    }

    #[test]
    fn translation_fallbacks_work() {
        let bundle = TranslationBundle::new(LocaleCode::Fr);
        assert_eq!(bundle.text("nav.dashboard", "Dash"), "Dashboard");
        assert_eq!(bundle.text("nav.missing_key", "Default"), "Default");
    }

    #[test]
    fn rtl_flag_honours_locale_metadata() {
        assert!(TranslationBundle::new(LocaleCode::Ar).rtl());
        assert!(!TranslationBundle::new(LocaleCode::En).rtl());
    }
}
