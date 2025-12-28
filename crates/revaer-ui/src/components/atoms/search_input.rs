//! Debounced search input for filter toolbars.
//!
//! # Design
//! - Keep local input state for immediate typing feedback.
//! - Emit debounced values to the caller for shared state updates.

use crate::components::daisy::{DaisyColor, DaisySize, tone_class};
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
    pub input_class: Classes,
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

    let size = props.size.with_prefix("input");
    let tone = tone_class("input", props.tone);
    let label_classes = {
        let mut classes = classes!("input", size, props.class.clone());
        if let Some(tone) = tone {
            classes.push(tone);
        }
        classes
    };

    html! {
        <label
            class={label_classes}>
            <span class="iconify lucide--search text-base-content/80 size-3.5"></span>
            <input
                class={classes!(
                    "text-base",
                    "placeholder:text-sm",
                    props.input_class.clone()
                )}
                type="search"
                placeholder={props.placeholder.clone()}
                value={AttrValue::from((*value_state).clone())}
                aria-label={props.aria_label.clone()}
                disabled={props.disabled}
                oninput={Callback::from(move |event: InputEvent| {
                    if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                        oninput.emit(input.value());
                    }
                })}
                ref={props.input_ref.clone()}
            />
        </label>
    }
}
