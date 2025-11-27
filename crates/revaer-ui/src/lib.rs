//! Revaer Web UI (Phase 1) scaffolding.
//! This crate holds the Yew front-end entrypoint plus shared tokens and locale metadata.

pub mod breakpoints;
pub mod i18n;
pub mod theme;

/// UI surface mode toggle. Defaults to [`UiMode::Simple`] for first-run users.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMode {
    Simple,
    Advanced,
}

/// Density preference for tables and cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Compact,
    Normal,
    Comfy,
}

impl Density {
    pub const fn all() -> [Self; 3] {
        [Self::Compact, Self::Normal, Self::Comfy]
    }
}

/// Quick description of supported layout panes used across dashboard and torrent detail views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Files,
    Peers,
    Trackers,
    Log,
    Info,
}

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod components;

#[cfg(target_arch = "wasm32")]
pub use app::run_app;
