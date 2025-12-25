use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Navigation icon for the categories route.
#[function_component(CategoriesIcon)]
pub(crate) fn categories_icon(props: &IconProps) -> Html {
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
                d="M4 6h16v4H4zM4 14h10v4H4z"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            />
        </svg>
    }
}
