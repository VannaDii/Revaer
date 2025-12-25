use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Navigation icon for the not-found route.
#[function_component(NotFoundIcon)]
pub(crate) fn not_found_icon(props: &IconProps) -> Html {
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
                d="M4 4h16v16H4z"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M8 8l8 8M16 8l-8 8"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </svg>
    }
}
