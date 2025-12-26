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
            class: Some("text-error hover:bg-error/10"),
        }
    }
}

pub(crate) fn render_action_menu(bundle: &TranslationBundle, items: Vec<ActionMenuItem>) -> Html {
    if items.is_empty() {
        return html! {};
    }
    html! {
        <div class="dropdown dropdown-end">
            <button
                type="button"
                tabindex="0"
                aria-label={bundle.text("torrents.more", "More")}
                class="btn btn-ghost btn-xs btn-square">
                <span class="iconify lucide--more-horizontal size-4"></span>
            </button>
            <ul
                tabindex="0"
                class="dropdown-content menu bg-base-100 rounded-box w-44 p-1 shadow">
                {for items.into_iter().map(|item| {
                    let class = classes!("justify-start", item.class);
                    html! {
                        <li>
                            <button
                                type="button"
                                class={class}
                                onclick={item.on_click}>
                                {item.label}
                            </button>
                        </li>
                    }
                })}
            </ul>
        </div>
    }
}
