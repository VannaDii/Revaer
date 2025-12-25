//! Debounced search input for filter toolbars.
//!
//! # Design
//! - Keep local input state for immediate typing feedback.
//! - Emit debounced values to the caller for shared state updates.

use crate::components::daisy::{DaisyColor, DaisySize, Input};
use gloo_timers::callback::Timeout;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct SearchInputProps {
    #[prop_or_default]
    pub value: AttrValue,
    #[prop_or_default]
    pub placeholder: Option<AttrValue>,
    #[prop_or_default]
    pub aria_label: Option<AttrValue>,
    #[prop_or_default]
    pub tone: Option<DaisyColor>,
    #[prop_or(DaisySize::Md)]
    pub size: DaisySize,
    #[prop_or(250)]
    pub debounce_ms: u32,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub input_ref: NodeRef,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub on_search: Callback<String>,
}

#[function_component(SearchInput)]
pub(crate) fn search_input(props: &SearchInputProps) -> Html {
    let value_state = use_state(|| props.value.to_string());
    let debounce = props.debounce_ms;
    let timer = use_mut_ref(|| None as Option<Timeout>);

    {
        let value_state = value_state.clone();
        let incoming = props.value.clone();
        use_effect_with_deps(
            move |incoming| {
                let next = incoming.to_string();
                if *value_state != next {
                    value_state.set(next);
                }
                || ()
            },
            incoming,
        );
    }

    let oninput = {
        let on_search = props.on_search.clone();
        let value_state = value_state.clone();
        let timer = timer.clone();
        Callback::from(move |next: String| {
            value_state.set(next.clone());
            if debounce == 0 {
                on_search.emit(next);
                return;
            }
            if let Some(timeout) = timer.borrow_mut().take() {
                drop(timeout);
            }
            let on_search = on_search.clone();
            *timer.borrow_mut() = Some(Timeout::new(debounce, move || {
                on_search.emit(next);
            }));
        })
    };

    html! {
        <Input
            value={AttrValue::from((*value_state).clone())}
            placeholder={props.placeholder.clone()}
            aria_label={props.aria_label.clone()}
            input_type={Some(AttrValue::from("search"))}
            tone={props.tone}
            size={props.size}
            disabled={props.disabled}
            input_ref={props.input_ref.clone()}
            class={props.class.clone()}
            oninput={oninput}
        />
    }
}
