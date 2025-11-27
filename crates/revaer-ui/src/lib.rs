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

pub mod breakpoints;
pub mod i18n;
pub mod logic;
pub mod models;
pub mod state;
pub mod theme;

#[cfg(target_arch = "wasm32")]
pub mod services;

/// UI surface mode toggle. Defaults to [`UiMode::Simple`] for first-run users.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMode {
    /// Simplified view with minimal controls.
    Simple,
    /// Full advanced controls exposed.
    Advanced,
}

/// Density preference for tables and cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    /// Compact rows (tight padding).
    Compact,
    /// Default spacing.
    Normal,
    /// Comfortable spacing for readability.
    Comfy,
}

impl Density {
    /// All supported density presets for toggle controls.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Compact, Self::Normal, Self::Comfy]
    }
}

/// Quick description of supported layout panes used across dashboard and torrent detail views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// Files pane.
    Files,
    /// Peers pane.
    Peers,
    /// Trackers pane.
    Trackers,
    /// Log pane.
    Log,
    /// Info pane.
    Info,
}

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod components;

#[cfg(target_arch = "wasm32")]
pub use app::run_app;

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
