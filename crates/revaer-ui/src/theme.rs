//! Brand palette and design tokens for the Revaer Web UI.

/// A single color token with a stable name and hex value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorToken {
    /// Semantic identifier for the shade (e.g., "500").
    pub name: &'static str,
    /// Hex RGB value for the shade.
    pub hex: &'static str,
}

/// Collection of related tokens (e.g., primary shades).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Palette identifier.
    pub id: &'static str,
    /// Ordered list of shades from lightest to darkest.
    pub shades: &'static [ColorToken],
}

/// Primary brand palette.
pub const PRIMARY: Palette = Palette {
    id: "primary",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#E7EFF4",
        },
        ColorToken {
            name: "100",
            hex: "#C2D6E4",
        },
        ColorToken {
            name: "200",
            hex: "#9CBBD3",
        },
        ColorToken {
            name: "300",
            hex: "#76A0C2",
        },
        ColorToken {
            name: "400",
            hex: "#4F85B1",
        },
        ColorToken {
            name: "500",
            hex: "#265D81",
        },
        ColorToken {
            name: "600",
            hex: "#1F4D6A",
        },
        ColorToken {
            name: "700",
            hex: "#183C52",
        },
        ColorToken {
            name: "800",
            hex: "#112B3A",
        },
        ColorToken {
            name: "900",
            hex: "#0A1B23",
        },
    ],
};

/// Secondary palette used for accents and elevated surfaces.
pub const SECONDARY: Palette = Palette {
    id: "secondary",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#F0EBF5",
        },
        ColorToken {
            name: "100",
            hex: "#DAD1E7",
        },
        ColorToken {
            name: "200",
            hex: "#C3B5D8",
        },
        ColorToken {
            name: "300",
            hex: "#A997C7",
        },
        ColorToken {
            name: "400",
            hex: "#8E78B4",
        },
        ColorToken {
            name: "500",
            hex: "#775A96",
        },
        ColorToken {
            name: "600",
            hex: "#60497A",
        },
        ColorToken {
            name: "700",
            hex: "#4C3962",
        },
        ColorToken {
            name: "800",
            hex: "#372A48",
        },
        ColorToken {
            name: "900",
            hex: "#241C2F",
        },
    ],
};

/// Accent palette for interactive elements and callouts.
pub const ACCENT: Palette = Palette {
    id: "accent",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#E6F2FB",
        },
        ColorToken {
            name: "100",
            hex: "#C0DFF8",
        },
        ColorToken {
            name: "200",
            hex: "#97C8F2",
        },
        ColorToken {
            name: "300",
            hex: "#6DAFEC",
        },
        ColorToken {
            name: "400",
            hex: "#4497E4",
        },
        ColorToken {
            name: "500",
            hex: "#258BD3",
        },
        ColorToken {
            name: "600",
            hex: "#1F78B5",
        },
        ColorToken {
            name: "700",
            hex: "#196391",
        },
        ColorToken {
            name: "800",
            hex: "#134C6C",
        },
        ColorToken {
            name: "900",
            hex: "#0D3549",
        },
    ],
};

/// Neutral palette for light theme surfaces and borders.
pub const NEUTRALS_LIGHT: Palette = Palette {
    id: "neutral",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#FFFFFF",
        },
        ColorToken {
            name: "100",
            hex: "#F8F9FA",
        },
        ColorToken {
            name: "150",
            hex: "#F1F3F5",
        },
        ColorToken {
            name: "200",
            hex: "#E9ECEF",
        },
        ColorToken {
            name: "250",
            hex: "#DFE3E6",
        },
        ColorToken {
            name: "300",
            hex: "#DEE2E6",
        },
        ColorToken {
            name: "400",
            hex: "#CED4DA",
        },
        ColorToken {
            name: "500",
            hex: "#ADB5BD",
        },
        ColorToken {
            name: "600",
            hex: "#6C757D",
        },
        ColorToken {
            name: "700",
            hex: "#495057",
        },
        ColorToken {
            name: "800",
            hex: "#343A40",
        },
        ColorToken {
            name: "900",
            hex: "#212529",
        },
    ],
};

/// Neutral palette for dark theme surfaces and borders.
pub const NEUTRALS_DARK: Palette = Palette {
    id: "dark",
    shades: &[
        ColorToken {
            name: "background",
            hex: "#121417",
        },
        ColorToken {
            name: "surface",
            hex: "#1A1C20",
        },
        ColorToken {
            name: "surface-raised",
            hex: "#1F2226",
        },
        ColorToken {
            name: "border",
            hex: "#2B2F34",
        },
        ColorToken {
            name: "text-primary",
            hex: "#F8F9FA",
        },
        ColorToken {
            name: "text-secondary",
            hex: "#C8CDD2",
        },
        ColorToken {
            name: "text-muted",
            hex: "#959DA6",
        },
        ColorToken {
            name: "primary-500",
            hex: "#4F85B1",
        },
        ColorToken {
            name: "primary-700",
            hex: "#2F526F",
        },
        ColorToken {
            name: "secondary-500",
            hex: "#A997C7",
        },
        ColorToken {
            name: "secondary-700",
            hex: "#6C5387",
        },
        ColorToken {
            name: "accent-500",
            hex: "#4497E4",
        },
        ColorToken {
            name: "accent-700",
            hex: "#1E5984",
        },
    ],
};

/// Success feedback palette.
pub const SUCCESS: Palette = Palette {
    id: "success",
    shades: &[
        ColorToken {
            name: "100",
            hex: "#D9F0EA",
        },
        ColorToken {
            name: "500",
            hex: "#2F9E7A",
        },
        ColorToken {
            name: "700",
            hex: "#1E6A51",
        },
    ],
};

/// Warning feedback palette.
pub const WARNING: Palette = Palette {
    id: "warning",
    shades: &[
        ColorToken {
            name: "100",
            hex: "#FFF4D8",
        },
        ColorToken {
            name: "500",
            hex: "#E2AC2F",
        },
        ColorToken {
            name: "700",
            hex: "#A4761A",
        },
    ],
};

/// Error feedback palette.
pub const ERROR: Palette = Palette {
    id: "error",
    shades: &[
        ColorToken {
            name: "100",
            hex: "#FCE6EE",
        },
        ColorToken {
            name: "500",
            hex: "#C43A61",
        },
        ColorToken {
            name: "700",
            hex: "#8E2643",
        },
    ],
};

/// Spacing scale in pixels.
pub const SPACING: [u8; 6] = [4, 8, 12, 16, 24, 32];
/// Corner radius tokens in pixels.
pub const RADII: [u8; 3] = [4, 8, 12];
/// Elevation tiers used for cards and drawers.
pub const ELEVATION: [&str; 3] = ["flat", "raised", "floating"];

/// Typographic scale names used by the CSS.
pub const TYPE_SCALE: [&str; 6] = ["xs", "sm", "md", "lg", "xl", "2xl"];

/// Light or dark theme preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    /// Light theme mode.
    Light,
    /// Dark theme mode.
    Dark,
}

impl ThemeMode {
    /// String identifier used in CSS datasets.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_have_expected_lengths() {
        assert_eq!(PRIMARY.shades.len(), 10);
        assert_eq!(ACCENT.shades.len(), 10);
        assert!(NEUTRALS_LIGHT.shades.len() >= 10);
        assert!(NEUTRALS_DARK.shades.len() >= 10);
    }

    #[test]
    fn theme_mode_to_str() {
        assert_eq!(ThemeMode::Light.as_str(), "light");
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
    }
}
