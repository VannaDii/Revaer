//! Sticky bulk action bar for list toolbars.
//!
//! # Design
//! - Keep labels/counts and actions fully prop-driven.
//! - Render action buttons through child content.

use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct BulkActionBarProps {
    pub select_label: AttrValue,
    pub selected_label: AttrValue,
    pub selected_count: usize,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub select_class: Classes,
    #[prop_or_default]
    pub on_toggle_all: Callback<MouseEvent>,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(BulkActionBar)]
pub(crate) fn bulk_action_bar(props: &BulkActionBarProps) -> Html {
    html! {
        <div
            role="alert"
            class={classes!(
                "alert",
                "bg-base-100",
                "border",
                "border-base-200",
                "shadow",
                "flex",
                "flex-wrap",
                "items-center",
                "gap-3",
                props.class.clone()
            )}>
            <button
                class={classes!("btn", "btn-sm", "btn-ghost", props.select_class.clone())}
                onclick={props.on_toggle_all.clone()}
                type="button">
                {props.select_label.clone()}
            </button>
            <span class="text-sm text-base-content/70">
                {format!("{} {}", props.selected_count, props.selected_label)}
            </span>
            <div class="ms-auto flex flex-wrap items-center gap-2">
                { for props.children.iter() }
            </div>
        </div>
    }
}
