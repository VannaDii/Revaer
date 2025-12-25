use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Navigation icon for the tags route.
#[function_component(TagsIcon)]
pub(crate) fn tags_icon(props: &IconProps) -> Html {
    let size = icon_size_attr(props.size);
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
            <path
                d="M4 8l8-4 8 4-8 12z"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            />
            <circle cx="12" cy="10" r="1.5" fill="currentColor" />
        </svg>
    }
}
