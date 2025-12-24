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
            hex: "#F9E6FF",
        },
        ColorToken {
            name: "100",
            hex: "#F1C8FF",
        },
        ColorToken {
            name: "200",
            hex: "#E1A1FF",
        },
        ColorToken {
            name: "300",
            hex: "#CF74F5",
        },
        ColorToken {
            name: "400",
            hex: "#B94AE1",
        },
        ColorToken {
            name: "500",
            hex: "#901CBB",
        },
        ColorToken {
            name: "600",
            hex: "#7A18A6",
        },
        ColorToken {
            name: "700",
            hex: "#631387",
        },
        ColorToken {
            name: "800",
            hex: "#4C0F68",
        },
        ColorToken {
            name: "900",
            hex: "#350A4A",
        },
    ],
};

/// Secondary palette used for accents and elevated surfaces.
pub const SECONDARY: Palette = Palette {
    id: "secondary",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#F5D9F4",
        },
        ColorToken {
            name: "100",
            hex: "#E5A0E4",
        },
        ColorToken {
            name: "200",
            hex: "#C664C8",
        },
        ColorToken {
            name: "300",
            hex: "#A93BB1",
        },
        ColorToken {
            name: "400",
            hex: "#8A1D92",
        },
        ColorToken {
            name: "500",
            hex: "#6A0071",
        },
        ColorToken {
            name: "600",
            hex: "#54005A",
        },
        ColorToken {
            name: "700",
            hex: "#400044",
        },
        ColorToken {
            name: "800",
            hex: "#2C002F",
        },
        ColorToken {
            name: "900",
            hex: "#1A001B",
        },
    ],
};

/// Accent palette for interactive elements and callouts.
pub const ACCENT: Palette = Palette {
    id: "accent",
    shades: &[
        ColorToken {
            name: "50",
            hex: "#FCE6FB",
        },
        ColorToken {
            name: "100",
            hex: "#F8C2F3",
        },
        ColorToken {
            name: "200",
            hex: "#F093EA",
        },
        ColorToken {
            name: "300",
            hex: "#E45DE0",
        },
        ColorToken {
            name: "400",
            hex: "#D33AD1",
        },
        ColorToken {
            name: "500",
            hex: "#C42AC3",
        },
        ColorToken {
            name: "600",
            hex: "#A61FA6",
        },
        ColorToken {
            name: "700",
            hex: "#871787",
        },
        ColorToken {
            name: "800",
            hex: "#671169",
        },
        ColorToken {
            name: "900",
            hex: "#490B4B",
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
            hex: "#FBFAFF",
        },
        ColorToken {
            name: "150",
            hex: "#F4F1FF",
        },
        ColorToken {
            name: "200",
            hex: "#EDE8FF",
        },
        ColorToken {
            name: "250",
            hex: "#E2DBF7",
        },
        ColorToken {
            name: "300",
            hex: "#D7D1F0",
        },
        ColorToken {
            name: "400",
            hex: "#C3BDE3",
        },
        ColorToken {
            name: "500",
            hex: "#A79CC9",
        },
        ColorToken {
            name: "600",
            hex: "#7C72A1",
        },
        ColorToken {
            name: "700",
            hex: "#5C5380",
        },
        ColorToken {
            name: "800",
            hex: "#3C355C",
        },
        ColorToken {
            name: "900",
            hex: "#201A3B",
        },
    ],
};

/// Neutral palette for dark theme surfaces and borders.
pub const NEUTRALS_DARK: Palette = Palette {
    id: "dark",
    shades: &[
        ColorToken {
            name: "background",
            hex: "#000030",
        },
        ColorToken {
            name: "surface",
            hex: "#07073B",
        },
        ColorToken {
            name: "surface-raised",
            hex: "#0C0C46",
        },
        ColorToken {
            name: "border",
            hex: "#1C1C5B",
        },
        ColorToken {
            name: "text-primary",
            hex: "#F3EDFF",
        },
        ColorToken {
            name: "text-secondary",
            hex: "#C8BFE9",
        },
        ColorToken {
            name: "text-muted",
            hex: "#9187B8",
        },
        ColorToken {
            name: "primary-500",
            hex: "#901CBB",
        },
        ColorToken {
            name: "primary-700",
            hex: "#5D0F7A",
        },
        ColorToken {
            name: "secondary-500",
            hex: "#6A0071",
        },
        ColorToken {
            name: "secondary-700",
            hex: "#3E0042",
        },
        ColorToken {
            name: "accent-500",
            hex: "#C42AC3",
        },
        ColorToken {
            name: "accent-700",
            hex: "#8F1F8E",
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
