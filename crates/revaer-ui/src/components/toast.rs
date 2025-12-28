use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use crate::models::{Toast, ToastKind};
use gloo::timers::callback::Timeout;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub(crate) struct ToastHostProps {
    pub toasts: Vec<Toast>,
    pub on_dismiss: Callback<u64>,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(ToastHost)]
pub(crate) fn toast_host(props: &ToastHostProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key);
    {
        let toasts = props.toasts.clone();
        let on_dismiss = props.on_dismiss.clone();
        use_effect_with_deps(
            move |list: &Vec<Toast>| {
                let mut handles = Vec::new();
                for toast in list.iter() {
                    let on_dismiss = on_dismiss.clone();
                    let id = toast.id;
                    handles.push(Timeout::new(4000, move || on_dismiss.emit(id)));
                }
                move || drop(handles)
            },
            toasts,
        );
    }

    html! {
        <div
            class={classes!("toast", "toast-end", "toast-bottom", props.class.clone())}
            aria-live="polite"
            aria-atomic="true"
        >
            {for props.toasts.iter().map(|toast| render_toast(toast, props.on_dismiss.clone(), t("toast.dismiss")))}
        </div>
    }
}

fn render_toast(toast: &Toast, on_dismiss: Callback<u64>, dismiss_label: String) -> Html {
    let class = match toast.kind {
        ToastKind::Info => "alert-info",
        ToastKind::Success => "alert-success",
        ToastKind::Error => "alert-error",
    };
    let id = toast.id;
    let on_close = {
        let on_dismiss = on_dismiss.clone();
        Callback::from(move |_| on_dismiss.emit(id))
    };

    html! {
        <div role="status" class={classes!("alert", class, "shadow")}>
            <span class="text-sm">{toast.message.clone()}</span>
            <button
                class="btn btn-ghost btn-xs btn-circle"
                aria-label={dismiss_label.clone()}
                onclick={on_close}
            >
                <span class="iconify lucide--x size-3.5"></span>
            </button>
        </div>
    }
}
