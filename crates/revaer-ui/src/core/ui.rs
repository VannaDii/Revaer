//! UI primitives shared across the crate (layout modes, density, panes).

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
    /// Overview pane.
    Overview,
    /// Files pane.
    Files,
    /// Options pane.
    Options,
}

#[cfg(test)]
mod tests {
    use super::{Density, Pane, UiMode};

    #[test]
    fn density_all_returns_expected_order() {
        assert_eq!(
            Density::all(),
            [Density::Compact, Density::Normal, Density::Comfy]
        );
    }

    #[test]
    fn pane_variants_are_distinct() {
        assert_ne!(Pane::Overview, Pane::Files);
        assert_ne!(Pane::Files, Pane::Options);
        assert_ne!(Pane::Options, Pane::Overview);
        assert_eq!(UiMode::Simple, UiMode::Simple);
    }
}
