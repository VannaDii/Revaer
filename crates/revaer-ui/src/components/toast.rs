use crate::i18n::{DEFAULT_LOCALE, TranslationBundle};
use gloo::timers::callback::Timeout;
use yew::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub kind: ToastKind,
}

#[derive(Properties, PartialEq)]
pub struct ToastHostProps {
    pub toasts: Vec<Toast>,
    pub on_dismiss: Callback<u64>,
}

#[function_component(ToastHost)]
pub fn toast_host(props: &ToastHostProps) -> Html {
    let bundle = use_context::<TranslationBundle>()
        .unwrap_or_else(|| TranslationBundle::new(DEFAULT_LOCALE));
    let t = |key: &str| bundle.text(key, "");
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
        <div class="toast-host" aria-live="polite" aria-atomic="true">
            {for props.toasts.iter().map(|toast| render_toast(toast, props.on_dismiss.clone(), t("toast.dismiss")))}
        </div>
    }
}

fn render_toast(toast: &Toast, on_dismiss: Callback<u64>, dismiss_label: String) -> Html {
    let class = match toast.kind {
        ToastKind::Info => "info",
        ToastKind::Success => "success",
        ToastKind::Error => "error",
    };
    let id = toast.id;
    let on_close = {
        let on_dismiss = on_dismiss.clone();
        Callback::from(move |_| on_dismiss.emit(id))
    };

    html! {
        <div class={classes!("toast", class)} role="status">
            <span>{toast.message.clone()}</span>
            <button class="ghost" aria-label={dismiss_label.clone()} onclick={on_close}>{"✕"}</button>
        </div>
    }
}
