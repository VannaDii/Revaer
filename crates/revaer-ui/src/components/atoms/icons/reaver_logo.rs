use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Brand mark used in the sidebar header.
#[function_component(RevaerLogoIcon)]
pub(crate) fn revaer_logo_icon(props: &IconProps) -> Html {
    let size = icon_size_attr(props.size);
    html! {
        <svg
            class={props.class.clone()}
            width={size.clone()}
            height={size}
            viewBox="0 0 64 64"
            aria-hidden={icon_aria_hidden(&props.title)}
            role={icon_role(&props.title)}
        >
            {icon_title(&props.title)}
            <path
                d="M18 14h18c7.2 0 12 4.8 12 11.2 0 5.6-3.6 9.6-8.8 10.8L44 50H33.6l-4.4-12H26V50H18zm18 16c3.2 0 5-1.6 5-4s-1.8-4-5-4H26v8z"
                fill="currentColor"
            />
        </svg>
    }
}
