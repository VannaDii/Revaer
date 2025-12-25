use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Navigation icon for the torrents route.
#[function_component(TorrentsIcon)]
pub(crate) fn torrents_icon(props: &IconProps) -> Html {
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
                d="M4 7l8-4 8 4v10l-8 4-8-4z"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M8 12l4 2 4-2"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            />
        </svg>
    }
}
