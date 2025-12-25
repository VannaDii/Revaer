use yew::prelude::*;

use super::{IconProps, icon_aria_hidden, icon_role, icon_size_attr, icon_title};

/// Right-pointing chevron icon.
#[function_component(ChevronRightIcon)]
pub(crate) fn chevron_right_icon(props: &IconProps) -> Html {
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
                d="M9 6l6 6-6 6"
                fill="none"
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
            />
        </svg>
    }
}
