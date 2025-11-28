use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Event, HtmlDivElement};
use yew::prelude::*;

use crate::logic::compute_window;

/// Minimal vertical virtualization helper to keep 50k+ rows responsive.
#[derive(Properties, PartialEq)]
pub(crate) struct VirtualListProps {
    pub len: usize,
    pub row_height: u32,
    #[prop_or(4)]
    pub overscan: u32,
    #[prop_or_default]
    pub height: Option<String>,
    pub render: Callback<usize, Html>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(VirtualList)]
pub(crate) fn virtual_list(props: &VirtualListProps) -> Html {
    let viewport_height = use_state(|| 0u32);
    let scroll_top = use_state(|| 0u32);
    let container_ref = use_node_ref();

    {
        let viewport_height = viewport_height.clone();
        let container_ref = container_ref.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(div) = container_ref.cast::<HtmlDivElement>() {
                    viewport_height.set(div.client_height() as u32);
                }
                || ()
            },
            props.height.clone(),
        );
    }

    let onscroll = {
        let scroll_top = scroll_top.clone();
        Callback::from(move |event: Event| {
            if let Some(div) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlDivElement>().ok())
            {
                scroll_top.set(div.scroll_top().max(0) as u32);
            }
        })
    };

    // Recompute viewport height on resize for responsive layouts.
    {
        let container_ref = container_ref.clone();
        let viewport_height = viewport_height.clone();
        use_effect(move || {
            let handler = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::UiEvent| {
                if let Some(div) = container_ref.cast::<HtmlDivElement>() {
                    viewport_height.set(div.client_height() as u32);
                }
            }) as Box<dyn FnMut(_)>);

            if let Some(window) = web_sys::window() {
                let _ = window
                    .add_event_listener_with_callback("resize", handler.as_ref().unchecked_ref());
            }

            move || {
                if let Some(window) = web_sys::window() {
                    let _ = window.remove_event_listener_with_callback(
                        "resize",
                        handler.as_ref().unchecked_ref(),
                    );
                }
            }
        });
    }

    let row_height = props.row_height.max(1);
    let (start, end, offset, total_height) = compute_window(
        *viewport_height,
        *scroll_top,
        row_height,
        props.len,
        props.overscan,
    );

    html! {
        <div
            ref={container_ref}
            class={classes!("virtual-list", props.class.clone())}
            style={format!("overflow-y:auto; position:relative; height:{};", props.height.clone().unwrap_or_else(|| "70vh".to_string()))}
            {onscroll}
            role="presentation"
        >
            <div style={format!("height:{}px; position:relative;", total_height)} aria-hidden="true"></div>
            <div style={format!("position:absolute; top:{}px; left:0; right:0;", offset)}>
                {for (start..end).map(|idx| props.render.emit(idx))}
            </div>
        </div>
    }
}
