//! Arrow icon rendering helpers.

use yew::prelude::*;

use super::{IconVariant, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Directional arrow orientations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArrowDirection {
    /// Upward-facing arrow.
    Up,
    /// Downward-facing arrow.
    Down,
}

/// Props for the directional arrow icon.
#[derive(Properties, PartialEq)]
pub(crate) struct ArrowIconProps {
    /// Arrow direction.
    #[prop_or(ArrowDirection::Up)]
    pub direction: ArrowDirection,
    /// Outline or solid rendering.
    #[prop_or(IconVariant::Solid)]
    pub variant: IconVariant,
    /// Icon size in pixels.
    #[prop_or(24)]
    pub size: u32,
    /// Additional CSS classes for the SVG.
    #[prop_or_default]
    pub class: Classes,
    /// Optional accessible title.
    #[prop_or_default]
    pub title: Option<AttrValue>,
}

#[function_component(ArrowIcon)]
pub(crate) fn arrow_icon(props: &ArrowIconProps) -> Html {
    let size = icon_size_attr(props.size);
    let path = match props.direction {
        ArrowDirection::Up => "M12 4l6 6h-4v6h-4v-6H6z",
        ArrowDirection::Down => "M12 20l-6-6h4V8h4v6h4z",
    };
    let glyph = match props.variant {
        IconVariant::Solid => html! {
            <path d={path} fill="currentColor" />
        },
        IconVariant::Outline => html! {
            <path
                d={path}
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linejoin="round"
            />
        },
    };

    html! {
        <svg
            class={props.class.clone()}
            width={size.clone()}
            height={size}
            viewBox="0 0 24 24"
            aria-hidden={icon_aria_hidden(&props.title)}
            role={icon_role(&props.title)}
        >
            {icon_title(&props.title)}
            {glyph}
        </svg>
    }
}
