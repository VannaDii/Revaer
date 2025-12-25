//! Empty state panel for list-like views.
//!
//! # Design
//! - Keep copy and actions entirely prop-driven.
//! - Render optional actions only when provided.

use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct EmptyStateProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub description: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(EmptyState)]
pub(crate) fn empty_state(props: &EmptyStateProps) -> Html {
    let has_actions = props.children.iter().next().is_some();
    html! {
        <div class={classes!("empty-state", props.class.clone())}>
            <h4>{props.title.clone()}</h4>
            {props.description.clone().map(|text| html! {
                <p class="muted">{text}</p>
            }).unwrap_or_default()}
            {if has_actions {
                html! { <div class="empty-actions">{ for props.children.iter() }</div> }
            } else {
                html! {}
            }}
        </div>
    }
}
