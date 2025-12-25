//! Action menu helpers shared across torrent list and detail views.
//!
//! # Design
//! - Keep menu rendering stateless and driven by caller-supplied items.
//! - Emit callbacks only; no side effects or state are stored here.

use crate::i18n::TranslationBundle;
use web_sys::MouseEvent;
use yew::prelude::*;

#[derive(Clone)]
pub(crate) struct ActionMenuItem {
    label: String,
    on_click: Callback<MouseEvent>,
    class: Option<&'static str>,
}

impl ActionMenuItem {
    pub(crate) fn new(label: String, on_click: Callback<MouseEvent>) -> Self {
        Self {
            label,
            on_click,
            class: None,
        }
    }

    pub(crate) fn danger(label: String, on_click: Callback<MouseEvent>) -> Self {
        Self {
            label,
            on_click,
            class: Some("danger"),
        }
    }
}

pub(crate) fn render_action_menu(bundle: &TranslationBundle, items: Vec<ActionMenuItem>) -> Html {
    if items.is_empty() {
        return html! {};
    }
    html! {
        <details class="row-menu">
            <summary class="ghost">{bundle.text("torrents.more", "More...")}</summary>
            <div class="menu">
                {for items.into_iter().map(|item| {
                    let class = classes!("ghost", item.class);
                    html! {
                        <button type="button" class={class} onclick={item.on_click}>{item.label}</button>
                    }
                })}
            </div>
        </details>
    }
}
