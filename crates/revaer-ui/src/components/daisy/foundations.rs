use yew::prelude::*;
use yew::virtual_dom::VTag;

/// Shared DaisyUI color tokens used by multiple components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaisyColor {
    Primary,
    Secondary,
    Accent,
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}

impl DaisyColor {
    /// Returns the class suffix (e.g. `"primary"`) for the color.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Accent => "accent",
            Self::Neutral => "neutral",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Common sizing tokens used by DaisyUI controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DaisySize {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl DaisySize {
    /// Returns the suffix used by DaisyUI for the selected size.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
        }
    }

    /// Adds a prefix (e.g. `btn`) to the size suffix for class composition.
    #[must_use]
    pub fn with_prefix(self, prefix: &str) -> String {
        format!("{prefix}-{}", self.suffix())
    }
}

/// Variants used across button-like elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaisyVariant {
    Solid,
    Outline,
    Ghost,
    Link,
    Soft,
}

impl Default for DaisyVariant {
    fn default() -> Self {
        Self::Solid
    }
}

impl DaisyVariant {
    /// Maps the variant to the DaisyUI class name.
    #[must_use]
    pub const fn as_class(self) -> Option<&'static str> {
        match self {
            Self::Solid => None,
            Self::Outline => Some("btn-outline"),
            Self::Ghost => Some("btn-ghost"),
            Self::Link => Some("btn-link"),
            Self::Soft => Some("btn-soft"),
        }
    }
}

/// Minimal common props shared by most stateless container wrappers.
#[derive(Properties, PartialEq)]
pub struct BasicProps {
    #[prop_or_default]
    pub id: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

/// Convenience helper for composing class lists with an optional tone.
#[must_use]
pub fn tone_class(prefix: &str, tone: Option<DaisyColor>) -> Option<String> {
    tone.map(|color| format!("{prefix}-{}", color.as_str()))
}

/// Utility to merge a base class with any consumer-provided classes.
#[must_use]
pub fn merge_classes(base: &'static str, extra: &Classes) -> Classes {
    if extra.is_empty() {
        Classes::from(base)
    } else {
        let mut classes = Classes::from(base);
        classes.push(extra.clone());
        classes
    }
}

/// Renders a simple tag with a base DaisyUI class and any custom content.
#[must_use]
pub fn render_container(tag: &'static str, base_class: &'static str, props: &BasicProps) -> Html {
    let mut node = VTag::new(tag);
    if let Some(id) = &props.id {
        node.add_attribute("id", id.to_string());
    }
    let classes = merge_classes(base_class, &props.class);
    node.add_attribute("class", classes.to_string());
    for child in props.children.iter() {
        node.add_child(child);
    }
    node.into()
}
