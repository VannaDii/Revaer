//! Shared SVG icon components for the UI shell.

use yew::prelude::*;

pub(crate) mod arrow;
mod categories;
mod chevron_right;
mod health;
mod not_found;
mod reaver_logo;
mod settings;
mod tags;
mod torrents;

pub(crate) use arrow::{ArrowDirection, ArrowIcon};
pub(crate) use categories::CategoriesIcon;
pub(crate) use chevron_right::ChevronRightIcon;
pub(crate) use health::HealthIcon;
pub(crate) use not_found::NotFoundIcon;
pub(crate) use reaver_logo::RevaerLogoIcon;
pub(crate) use settings::SettingsIcon;
pub(crate) use tags::TagsIcon;
pub(crate) use torrents::TorrentsIcon;

/// Variant styling for icons that support outline or solid rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconVariant {
    /// Stroke-first outline style.
    Outline,
    /// Solid fill style.
    Solid,
}

/// Common icon props shared by simple SVG components.
#[derive(Properties, PartialEq)]
pub(crate) struct IconProps {
    /// Icon size in pixels (applies to both width and height).
    #[prop_or(24)]
    pub size: u32,
    /// Extra classes applied to the SVG element.
    #[prop_or_default]
    pub class: Classes,
    /// Optional accessible title for the SVG.
    #[prop_or_default]
    pub title: Option<AttrValue>,
}

fn icon_title(title: &Option<AttrValue>) -> Html {
    title
        .as_ref()
        .map(|text| html! { <title>{text.clone()}</title> })
        .unwrap_or_default()
}

fn icon_aria_hidden(title: &Option<AttrValue>) -> AttrValue {
    if title.is_some() {
        AttrValue::from("false")
    } else {
        AttrValue::from("true")
    }
}

fn icon_role(title: &Option<AttrValue>) -> AttrValue {
    if title.is_some() {
        AttrValue::from("img")
    } else {
        AttrValue::from("presentation")
    }
}

fn icon_size_attr(size: u32) -> AttrValue {
    AttrValue::from(size.to_string())
}
