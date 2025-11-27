use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct AuthPromptProps {
    pub require_key: bool,
    pub allow_anonymous: bool,
    pub on_submit: Callback<Option<String>>,
}

#[function_component(AuthPrompt)]
pub fn auth_prompt(props: &AuthPromptProps) -> Html {
    let api_key = use_state(String::new);
    let error = use_state(|| None as Option<String>);

    let submit = {
        let api_key = api_key.clone();
        let error = error.clone();
        let on_submit = props.on_submit.clone();
        Callback::from(move |_| {
            if api_key.is_empty() && props.require_key && !props.allow_anonymous {
                error.set(Some("API key required for remote mode".to_string()));
                return;
            }
            error.set(None);
            let value = if api_key.is_empty() {
                None
            } else {
                Some((*api_key).clone())
            };
            on_submit.emit(value);
        })
    };

    let on_input = {
        let api_key = api_key.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                api_key.set(input.value());
            }
        })
    };

    html! {
        <div class="auth-overlay" role="dialog" aria-modal="true">
            <div class="card">
                <header>
                    <h3>{"API access required"}</h3>
                </header>
                <p class="muted">
                    {"Enter your API key to connect. Remote mode always requires a key; LAN mode may allow anonymous if enabled by the backend."}
                </p>
                <label class="stack">
                    <span>{"API Key"}</span>
                    <input type="password" placeholder="revaer_api_key" onchange={on_input} />
                </label>
                {if props.allow_anonymous {
                    html! { <p class="muted">{"LAN mode detected: leave blank to continue anonymously."}</p> }
                } else { html!{} }}
                {if let Some(err) = &*error {
                    html! { <p class="error-text">{err}</p> }
                } else { html! {} }}
                <div class="actions">
                    <button class="ghost" onclick={{
                        let on_submit = props.on_submit.clone();
                        Callback::from(move |_| on_submit.emit(None))
                    }}>{"Use anonymous (if allowed)"}</button>
                    <button class="solid" onclick={submit}>{"Continue"}</button>
                </div>
            </div>
        </div>
    }
}
