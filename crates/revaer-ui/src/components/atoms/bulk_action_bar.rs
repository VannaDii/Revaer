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
        <div class={classes!("bulk-actions", props.class.clone())}>
            <button
                class={classes!("ghost", props.select_class.clone())}
                onclick={props.on_toggle_all.clone()}
            >
                {props.select_label.clone()}
            </button>
            <span class="muted">{format!("{} {}", props.selected_count, props.selected_label)}</span>
            <div class="bulk-buttons">
                { for props.children.iter() }
            </div>
        </div>
    }
}
