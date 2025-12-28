use crate::components::daisy::molecules::Modal;
use yew::prelude::*;

const CONFIRM_PHRASE: &str = "factory reset";

#[derive(Properties, PartialEq)]
pub(crate) struct FactoryResetModalProps {
    pub open: bool,
    pub busy: bool,
    pub on_confirm: Callback<String>,
    pub on_close: Callback<()>,
}

#[function_component(FactoryResetModal)]
pub(crate) fn factory_reset_modal(props: &FactoryResetModalProps) -> Html {
    let input_value = use_state(String::new);
    let touched = use_state(|| false);

    {
        let input_value = input_value.clone();
        let touched = touched.clone();
        use_effect_with_deps(
            move |open| {
                if *open {
                    input_value.set(String::new());
                    touched.set(false);
                }
                || ()
            },
            props.open,
        );
    }

    let on_input = {
        let input_value = input_value.clone();
        let touched = touched.clone();
        Callback::from(move |event: InputEvent| {
            if let Some(input) = event.target_dyn_into::<web_sys::HtmlInputElement>() {
                input_value.set(input.value());
                touched.set(true);
            }
        })
    };

    let confirm_matches = input_value.trim() == CONFIRM_PHRASE;
    let show_error = *touched && !confirm_matches && !input_value.trim().is_empty();
    let input_class = classes!(
        "input",
        "input-bordered",
        "w-full",
        show_error.then_some("input-error")
    );

    let on_confirm = {
        let input_value = input_value.clone();
        let on_confirm = props.on_confirm.clone();
        Callback::from(move |_| {
            let value = input_value.trim().to_string();
            if value == CONFIRM_PHRASE {
                on_confirm.emit(value);
            }
        })
    };

    html! {
        <Modal open={props.open} on_close={props.on_close.clone()}>
            <div class="space-y-4">
                <div>
                    <h3 class="text-lg font-semibold">{"Factory reset"}</h3>
                    <p class="text-sm text-base-content/70">
                        {"This will erase configuration and reset the system to setup mode."}
                    </p>
                </div>
                <div class="space-y-2">
                    <label class="form-control gap-1">
                        <span class="label-text text-xs">
                            {"Type the confirmation phrase to continue"}
                        </span>
                        <input
                            type="text"
                            class={input_class}
                            placeholder={CONFIRM_PHRASE}
                            value={(*input_value).clone()}
                            oninput={on_input}
                            disabled={props.busy}
                        />
                        <span class="label-text-alt text-xs text-base-content/60">
                            {"Required phrase: "}
                            <span class="font-mono">{CONFIRM_PHRASE}</span>
                        </span>
                    </label>
                    {if show_error {
                        html! {
                            <p class="text-xs text-error">{"Confirmation phrase does not match."}</p>
                        }
                    } else {
                        html! {}
                    }}
                </div>
                <div class="flex justify-end gap-2">
                    <button
                        class="btn btn-ghost btn-sm"
                        onclick={{
                            let on_close = props.on_close.clone();
                            Callback::from(move |_| on_close.emit(()))
                        }}
                        disabled={props.busy}
                    >
                        {"Cancel"}
                    </button>
                    <button
                        class="btn btn-error btn-sm"
                        onclick={on_confirm}
                        disabled={!confirm_matches || props.busy}
                    >
                        {"Factory reset"}
                    </button>
                </div>
            </div>
        </Modal>
    }
}
