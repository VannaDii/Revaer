use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Navigation icon for the health route.
#[function_component(HealthIcon)]
pub(crate) fn health_icon(props: &IconProps) -> Html {
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
                d="M5 12h4l2-4 3 8 2-4h3"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </svg>
    }
}
