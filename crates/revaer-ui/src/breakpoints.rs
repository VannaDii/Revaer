//! Responsive breakpoint definitions for the Web UI.

/// Individual breakpoint with an inclusive minimum width and optional maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Breakpoint {
    pub name: &'static str,
    pub min_width: u16,
    pub max_width: Option<u16>,
}

pub const XS: Breakpoint = Breakpoint {
    name: "xs",
    min_width: 0,
    max_width: Some(479),
};
pub const SM: Breakpoint = Breakpoint {
    name: "sm",
    min_width: 480,
    max_width: Some(767),
};
pub const MD: Breakpoint = Breakpoint {
    name: "md",
    min_width: 768,
    max_width: Some(1023),
};
pub const LG: Breakpoint = Breakpoint {
    name: "lg",
    min_width: 1024,
    max_width: Some(1439),
};
pub const XL: Breakpoint = Breakpoint {
    name: "xl",
    min_width: 1440,
    max_width: Some(1919),
};
pub const XXL: Breakpoint = Breakpoint {
    name: "2xl",
    min_width: 1920,
    max_width: None,
};

/// Ordered breakpoints used for layout decisions and CSS variable emission.
pub const BREAKPOINTS: [Breakpoint; 6] = [XS, SM, MD, LG, XL, XXL];

/// Find the first breakpoint matching the supplied width.
#[must_use]
pub fn for_width(width: u16) -> Breakpoint {
    BREAKPOINTS
        .iter()
        .copied()
        .find(|bp| width >= bp.min_width && bp.max_width.is_none_or(|max| width <= max))
        .unwrap_or(XXL)
}
